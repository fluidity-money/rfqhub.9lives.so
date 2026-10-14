-- Run in a fresh session against a disposable migrated database:
-- psql -X -v ON_ERROR_STOP=1 -f db/tests/available-balance.sql
-- All fixtures roll back; sequence advances are not transactional.
BEGIN;
SET LOCAL plpgsql.check_asserts = on;
CREATE TEMP TABLE rfqhub_events_balance_changed (
	id BIGSERIAL PRIMARY KEY, account_addr ADDRESS, asset ADDRESS,
	amount BIGINT, version BIGINT
);
-- Shadow the persistent view too: it otherwise remains bound to the real feed.
CREATE TEMP VIEW rfqhub_onramped_balances_1 AS
SELECT account_addr, asset, SUM(amount) AS amount
FROM rfqhub_events_balance_changed
GROUP BY account_addr, asset;

DO $$
DECLARE
	v_account ADDRESS := '0x00000000000000000000000000000000000000a1';
	v_other ADDRESS := '0x00000000000000000000000000000000000000a2';
	v_asset ADDRESS := '0x00000000000000000000000000000000000000a3';
	v_other_asset ADDRESS := '0x00000000000000000000000000000000000000a4';
	v_taker INTEGER;
	v_maker INTEGER;
	v_child INTEGER;
	v_ts INTEGER;
	v_case TEXT;
	v_i INTEGER := 0;
	v_expected BIGINT := 80;
	v_before JSONB;
	v_after JSONB;
BEGIN
	ASSERT (SELECT provolatile = 's' AND prorettype = 'bigint'::regtype
		FROM pg_proc WHERE oid = 'rfqhub_get_available_balance_1(address,address)'::regprocedure),
		'available balance must return BIGINT and be STABLE';
	ASSERT rfqhub_get_available_balance_1(v_account, v_asset) = 0, 'empty balance';
	INSERT INTO rfqhub_events_balance_changed (account_addr, asset, amount, version)
	VALUES (v_account, v_asset, 50, 1), (v_account, v_asset, 50, 2);
	ASSERT rfqhub_get_available_balance_1(v_account, v_asset) = 100, 'sum equal-valued onramp credits';

	-- The open child reserves 20 pot units and later models consumed sources.
	INSERT INTO rfqhub_bundles_taker_1 (
		acc_addr, user_id, asset, amt, pot_funded_amt, min_offer, is_up,
		price_target, outcome, ms_ts, deadline, expiry
	) VALUES (v_account, 100, v_asset, 20, 20, 0, TRUE, 100,
		'BitcoinPrice', 1, CURRENT_TIMESTAMP + INTERVAL '1 hour',
		CURRENT_TIMESTAMP + INTERVAL '2 hours') RETURNING id INTO v_child;
	ASSERT rfqhub_get_available_balance_1(v_account, v_asset) = 80, 'unreserved pot only';
	INSERT INTO rfqhub_timestamplets_1 (timestamplet_time, timestamplet_outcome, timestamplet_val)
	VALUES (CURRENT_TIMESTAMP - INTERVAL '1 hour', 'BitcoinPrice', 200)
	RETURNING id INTO v_ts;

	FOREACH v_case IN ARRAY ARRAY[
		'eligible', 'eligible second', 'unsigned taker', 'unsigned maker',
		'ledger consumed', 'legacy consumed', 'offramped', 'maker winner',
		'wrong account', 'wrong asset', 'zero winnings', 'null winnings',
		'unselected', 'null selection', 'unknown winner', 'no timestamplet',
		'no selected maker', 'mismatched parent'
	] LOOP
		v_i := v_i + 1;
		INSERT INTO rfqhub_bundles_taker_1 (
			acc_addr, user_id, asset, amt, min_offer, is_up, price_target,
			outcome, ms_ts, deadline, expiry, structure, end_source_amt, was_offramped
		) VALUES (
			CASE WHEN v_case = 'wrong account' THEN v_other ELSE v_account END,
			v_i, CASE WHEN v_case = 'wrong asset' THEN v_other_asset ELSE v_asset END,
			10, 0, TRUE, 100, 'BitcoinPrice', 1,
			CURRENT_TIMESTAMP - INTERVAL '2 hours', CURRENT_TIMESTAMP - INTERVAL '1 hour',
			CASE WHEN v_case = 'unsigned taker' THEN NULL ELSE '{"signed":"taker"}'::JSON END,
			CASE WHEN v_case = 'zero winnings' THEN 0 WHEN v_case = 'null winnings' THEN NULL ELSE 30 END,
			v_case = 'offramped'
		) RETURNING id INTO v_taker;
		INSERT INTO rfqhub_bundles_maker_1 (
			acc_addr, user_id, bundle_taker_id, ms_ts, amt, structure,
			was_chosen, was_correct, timestamplet_id, end_source_amt
		) VALUES (v_other, v_i,
			CASE WHEN v_case = 'mismatched parent' THEN v_child ELSE v_taker END,
			1, 20, CASE WHEN v_case = 'unsigned maker' THEN NULL ELSE '{"signed":"maker"}'::JSON END,
			CASE WHEN v_case = 'null selection' THEN NULL ELSE v_case <> 'unselected' END,
			CASE WHEN v_case = 'unknown winner' THEN NULL ELSE v_case = 'maker winner' END,
			CASE WHEN v_case = 'no timestamplet' THEN NULL ELSE v_ts END,
			CASE WHEN v_case = 'maker winner' THEN 30 ELSE 0 END
		) RETURNING id INTO v_maker;
		IF v_case <> 'no selected maker' THEN
			UPDATE rfqhub_bundles_taker_1 SET maker_id = v_maker WHERE id = v_taker;
		END IF;
		IF v_case = 'ledger consumed' THEN
			INSERT INTO rfqhub_bundles_taker_sources_1
				(bundle_taker_id, source_taker_id, source_amt, amt_to_onramp_amt)
			VALUES (v_child, v_taker, 10, 20);
		ELSIF v_case = 'legacy consumed' THEN
			-- A separate legacy child has no ledger entry and no pot reservation.
			INSERT INTO rfqhub_bundles_taker_1 (
				acc_addr, user_id, source, asset, amt, min_offer, is_up,
				price_target, outcome, ms_ts, deadline, expiry
			) VALUES (v_account, 101, v_taker, v_asset, 10, 0, TRUE,
				100, 'BitcoinPrice', 1, CURRENT_TIMESTAMP + INTERVAL '1 hour',
				CURRENT_TIMESTAMP + INTERVAL '2 hours');
		END IF;
		IF v_case IN ('eligible', 'eligible second') THEN
			v_expected := v_expected + 30;
		END IF;
		ASSERT rfqhub_get_available_balance_1(v_account, v_asset) = v_expected,
			'eligibility mismatch: ' || v_case;
		RAISE NOTICE 'PASS: available balance case %', v_case;
	END LOOP;
	ASSERT rfqhub_get_spendable_1(v_account, v_asset) = 80, 'spendable must remain pot-only';
	ASSERT rfqhub_get_available_balance_1(v_account, v_asset) = 140, 'pot plus multiple eligible winnings';
	-- Other account has exactly one eligible taker result; its maker winnings
	-- must not contribute (its pot is zero, regardless of maker reservations).
	ASSERT rfqhub_get_available_balance_1(v_other, v_asset) = 30, 'exclude maker winnings';
	ASSERT rfqhub_get_available_balance_1(v_account, v_other_asset) = 30, 'asset isolation';

	SELECT jsonb_build_array(
		(SELECT jsonb_agg(to_jsonb(t) ORDER BY id) FROM rfqhub_bundles_taker_1 t),
		(SELECT jsonb_agg(to_jsonb(m) ORDER BY id) FROM rfqhub_bundles_maker_1 m),
		(SELECT jsonb_agg(to_jsonb(s) ORDER BY source_taker_id) FROM rfqhub_bundles_taker_sources_1 s)
	) INTO v_before;
	FOR v_i IN 1..3 LOOP
		ASSERT rfqhub_get_available_balance_1(v_account, v_asset) = 140, 'repeatable read without consumption';
	END LOOP;
	SELECT jsonb_build_array(
		(SELECT jsonb_agg(to_jsonb(t) ORDER BY id) FROM rfqhub_bundles_taker_1 t),
		(SELECT jsonb_agg(to_jsonb(m) ORDER BY id) FROM rfqhub_bundles_maker_1 m),
		(SELECT jsonb_agg(to_jsonb(s) ORDER BY source_taker_id) FROM rfqhub_bundles_taker_sources_1 s)
	) INTO v_after;
	ASSERT v_before IS NOT DISTINCT FROM v_after, 'balance reads must not consume or modify sources';
	RAISE NOTICE 'PASS: zero, pure pot, reservations, multiple winnings, maker exclusion, no read consumption';
END;
$$;
ROLLBACK;
