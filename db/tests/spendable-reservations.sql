-- Run in a fresh session against a disposable database with the migration applied:
-- psql -X -v ON_ERROR_STOP=1 -f db/tests/spendable-reservations.sql
-- Direct fixtures model signed pot-funded offers; conclude and withdrawal use
-- the real functions. The temporary feed shadows the external event table.
-- All fixture changes roll back (sequence advances are not transactional).
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
	v_taker_account ADDRESS := '0x00000000000000000000000000000000000000f1';
	v_maker_account ADDRESS := '0x00000000000000000000000000000000000000f2';
	v_rejected_account ADDRESS := '0x00000000000000000000000000000000000000f3';
	v_asset ADDRESS := '0x00000000000000000000000000000000000000f4';
	v_taker INTEGER;
	v_maker INTEGER;
	v_rejected INTEGER;
	v_unmatched INTEGER;
	v_i INTEGER;
BEGIN
	INSERT INTO rfqhub_events_balance_changed (account_addr, asset, amount, version)
	VALUES (v_taker_account, v_asset, 50, 1), (v_taker_account, v_asset, 50, 2),
		(v_maker_account, v_asset, 50, 1), (v_maker_account, v_asset, 50, 2),
		(v_rejected_account, v_asset, 100, 1);

	-- An open auction reserves pot, but an expired auction with NO makers releases.
	INSERT INTO rfqhub_bundles_taker_1 (
		acc_addr, user_id, asset, amt, pot_funded_amt, min_offer, is_up,
		price_target, outcome, ms_ts, deadline, expiry, structure
	) VALUES (v_taker_account, 1, v_asset, 10, 10, 0, TRUE,
		100, 'BitcoinPrice', 1, CURRENT_TIMESTAMP + INTERVAL '1 hour',
		CURRENT_TIMESTAMP + INTERVAL '2 hours', '{"signed":"taker"}')
	RETURNING id INTO v_unmatched;
	ASSERT rfqhub_get_spendable_1(v_taker_account, v_asset) = 90,
		'open unmatched taker must reserve pot';
	UPDATE rfqhub_bundles_taker_1 SET deadline = CURRENT_TIMESTAMP - INTERVAL '1 second'
	WHERE id = v_unmatched;
	ASSERT rfqhub_get_spendable_1(v_taker_account, v_asset) = 100,
		'expired unmatched taker must release pot';

	-- Exercise both taker-winning and maker-winning actual conclusions, so both
	-- positive and zero end_source_amt values must retain the original pot stake.
	FOR v_i IN 1..2 LOOP
		INSERT INTO rfqhub_bundles_taker_1 (
			acc_addr, user_id, asset, amt, pot_funded_amt, min_offer, is_up,
			price_target, outcome, ms_ts, deadline, expiry, structure
		) VALUES (v_taker_account, v_i + 1, v_asset, 20, 20, 0, TRUE,
			100, 'BitcoinPrice', 1, CURRENT_TIMESTAMP - INTERVAL '2 hours',
			CURRENT_TIMESTAMP - v_i * INTERVAL '1 minute', '{"signed":"taker"}')
		RETURNING id INTO v_taker;
		INSERT INTO rfqhub_bundles_maker_1 (
			acc_addr, user_id, bundle_taker_id, ms_ts, amt, structure
		) VALUES (v_maker_account, v_i, v_taker, 1, 30, '{"signed":"maker"}')
		RETURNING id INTO v_maker;
		INSERT INTO rfqhub_bundles_maker_1 (
			acc_addr, user_id, bundle_taker_id, ms_ts, amt, structure
		) VALUES (v_rejected_account, v_i, v_taker, 1, 10, '{"signed":"rejected"}')
		RETURNING id INTO v_rejected;

		ASSERT rfqhub_get_spendable_1(v_taker_account, v_asset) = 100 - 20 * v_i,
			'expired auction with a maker must keep taker pot reserved';
		ASSERT rfqhub_get_spendable_1(v_maker_account, v_asset) = 100 - 30 * v_i,
			'pending maker must reserve pot';
		ASSERT rfqhub_get_spendable_1(v_rejected_account, v_asset) = 90,
			'not-yet-rejected offer must reserve pot';

		PERFORM * FROM rfqhub_conclude_aggregate_1(v_taker, CASE WHEN v_i = 1 THEN 200 ELSE 0 END);
		ASSERT (SELECT maker_id = v_maker AND end_source_amt = CASE WHEN v_i = 1 THEN 50 ELSE 0 END
			FROM rfqhub_bundles_taker_1 WHERE id = v_taker);
		ASSERT (SELECT was_chosen AND end_source_amt = CASE WHEN v_i = 1 THEN 0 ELSE 50 END
			FROM rfqhub_bundles_maker_1 WHERE id = v_maker);
		ASSERT (SELECT was_chosen = FALSE FROM rfqhub_bundles_maker_1 WHERE id = v_rejected);
		ASSERT rfqhub_get_spendable_1(v_taker_account, v_asset) = 100 - 20 * v_i,
			'conclude must not release taker pot';
		ASSERT rfqhub_get_spendable_1(v_maker_account, v_asset) = 100 - 30 * v_i,
			'conclude must not release selected maker pot';
		ASSERT rfqhub_get_spendable_1(v_rejected_account, v_asset) = 100,
			'rejected makers must release pot';

		PERFORM * FROM rfqhub_mark_winner_as_offramped_1(
			CASE WHEN v_i = 1 THEN v_taker_account ELSE v_maker_account END,
			v_i = 1, v_taker);
		IF v_i = 1 THEN
			ASSERT (SELECT was_offramped FROM rfqhub_bundles_taker_1 WHERE id = v_taker);
		ELSE
			ASSERT (SELECT was_offramped FROM rfqhub_bundles_maker_1 WHERE id = v_maker);
		END IF;
		ASSERT rfqhub_get_spendable_1(v_taker_account, v_asset) = 100 - 20 * v_i,
			'withdrawal request must not release taker pot';
		ASSERT rfqhub_get_spendable_1(v_maker_account, v_asset) = 100 - 30 * v_i,
			'withdrawal request must not release maker pot';
	END LOOP;

	-- Unrelated deposits for the SAME account/asset add credits, but contain
	-- no bundle link: never release matched stakes.
	INSERT INTO rfqhub_events_balance_changed (account_addr, asset, amount, version)
	VALUES (v_taker_account, v_asset, 10, 3), (v_maker_account, v_asset, 10, 3);
	ASSERT rfqhub_get_spendable_1(v_taker_account, v_asset) = 70,
		'unrelated BalanceChanged must not release taker reservations';
	ASSERT rfqhub_get_spendable_1(v_maker_account, v_asset) = 50,
		'unrelated BalanceChanged must not release maker reservations';

	-- Model missing indexed credits (fixture-only removal), not a debit event.
	-- Reservations above the indexed onramp sum retain the existing zero floor.
	DELETE FROM rfqhub_events_balance_changed
	WHERE account_addr IN (v_taker_account, v_maker_account) AND asset = v_asset;
	ASSERT rfqhub_get_spendable_1(v_taker_account, v_asset) = 0;
	ASSERT rfqhub_get_spendable_1(v_maker_account, v_asset) = 0;
	RAISE NOTICE 'PASS: conclude, withdrawal, unrelated balance events, expired unmatched, rejected makers, zero floor';
END;
$$;
ROLLBACK;
