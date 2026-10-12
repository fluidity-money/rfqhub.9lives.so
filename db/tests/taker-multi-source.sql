-- Run in a fresh psql session against a disposable database with the migration
-- applied: psql -X -v ON_ERROR_STOP=1 -f db/tests/taker-multi-source.sql
-- No oracle/event fixture schema is required: a temporary table shadows the
-- externally managed BalanceChanged feed. All changes roll back.
BEGIN;
CREATE TEMP TABLE rfqhub_events_balance_changed (
	id BIGSERIAL PRIMARY KEY, account_addr ADDRESS, asset ADDRESS, amount BIGINT
);
INSERT INTO rfqhub_events_balance_changed (account_addr, asset, amount)
VALUES ('0x00000000000000000000000000000000000000a1',
	'0x00000000000000000000000000000000000000b1', 100);

-- Seed genuinely concluded, signed taker winnings without relying on an oracle.
CREATE FUNCTION pg_temp.seed_winner(p_account ADDRESS, p_asset ADDRESS, p_amt BIGINT)
RETURNS INTEGER LANGUAGE plpgsql AS $$
DECLARE
	v_taker INTEGER;
	v_maker INTEGER;
	v_user INTEGER;
BEGIN
	INSERT INTO rfqhub_bundles_taker_user_count_1 AS c (addr, count)
	VALUES (p_account, 1) ON CONFLICT (addr) DO UPDATE SET count = c.count + 1
	RETURNING count INTO v_user;
	INSERT INTO rfqhub_bundles_taker_1 (
		acc_addr, user_id, asset, amt, min_offer, is_up, price_target,
		outcome, ms_ts, deadline, expiry, structure
	) VALUES (p_account, v_user, p_asset, 1, 0, TRUE, 100,
		'BitcoinPrice', 1, '2025-12-31', '2026-01-01', '{"signed":"taker"}')
	RETURNING id INTO v_taker;
	INSERT INTO rfqhub_bundles_maker_user_count_1 AS c (addr, count)
	VALUES ('0x00000000000000000000000000000000000000c1', 1)
	ON CONFLICT (addr) DO UPDATE SET count = c.count + 1 RETURNING count INTO v_user;
	INSERT INTO rfqhub_bundles_maker_1 (
		acc_addr, user_id, bundle_taker_id, ms_ts, amt, structure
	) VALUES ('0x00000000000000000000000000000000000000c1', v_user,
		v_taker, 1, p_amt - 1, '{"signed":"maker"}')
	RETURNING id INTO v_maker;
	PERFORM * FROM rfqhub_conclude_aggregate_1(v_taker, 200);
	RETURN v_taker;
END;
$$;

DO $$
DECLARE
	v_account ADDRESS := '0x00000000000000000000000000000000000000a1';
	v_asset ADDRESS := '0x00000000000000000000000000000000000000b1';
	v_deadline INTEGER := EXTRACT(EPOCH FROM CURRENT_TIMESTAMP + INTERVAL '1 hour')::INTEGER;
	v_expiry INTEGER := EXTRACT(EPOCH FROM CURRENT_TIMESTAMP + INTERVAL '2 hours')::INTEGER;
	v_small1 INTEGER;
	v_small2 INTEGER;
	v_big INTEGER;
	v_source INTEGER;
	v_bundle INTEGER;
	v_ids INTEGER[];
	v_takes BIGINT[];
	v_remainders BIGINT[];
	v_rows INTEGER;
	v_count INTEGER;
	v_before INTEGER;
	v_r RECORD;
BEGIN
	-- Insert the largest first so ordering by ID alone would fail.
	v_big := pg_temp.seed_winner(v_account, v_asset, 30);
	v_small1 := pg_temp.seed_winner(v_account, v_asset, 10);
	v_small2 := pg_temp.seed_winner(v_account, v_asset, 10);
	v_rows := 0;
	FOR v_r IN SELECT * FROM rfqhub_create_bundle_taker_1(
		v_account, v_asset, 25, 25, TRUE, 100, 'BitcoinPrice', 1, v_expiry, 0, v_deadline)
	LOOP
		v_rows := v_rows + 1;
		v_bundle := v_r.bundle_taker_id;
		v_ids := array_append(v_ids, (SELECT m.bundle_taker_id
			FROM rfqhub_bundles_maker_1 AS m WHERE m.id = v_r.source_maker_id));
		v_takes := array_append(v_takes, v_r.source_amt);
		v_remainders := array_append(v_remainders, (
			SELECT s.amt_to_onramp_amt FROM rfqhub_bundles_taker_sources_1 AS s
			WHERE s.source_taker_id = (SELECT m.bundle_taker_id
				FROM rfqhub_bundles_maker_1 AS m WHERE m.id = v_r.source_maker_id)));
		ASSERT v_r.amt = 25;
		ASSERT v_r.source_maker_id IS NOT NULL;
		ASSERT v_r.bundle_maker_structure::JSONB = '{"signed":"maker"}'::JSONB;
		ASSERT v_r.timestamplet_id IS NOT NULL AND v_r.timestamplet_end_value = 200;
		ASSERT v_r.timestamplet_time = '2026-01-01'::TIMESTAMP;
		ASSERT v_r.timestamplet_outcome = 'BitcoinPrice';
	END LOOP;
	ASSERT v_rows = 3 AND v_ids = ARRAY[v_small1, v_small2, v_big];
	ASSERT v_takes = ARRAY[10, 10, 5]::BIGINT[];
	ASSERT v_remainders = ARRAY[0, 0, 25]::BIGINT[];
	ASSERT (SELECT pot_funded_amt = 0 AND source IS NULL FROM rfqhub_bundles_taker_1 WHERE id = v_bundle);
	ASSERT (SELECT count(*) = 3 FROM rfqhub_bundles_taker_sources_1 WHERE bundle_taker_id = v_bundle);
	ASSERT rfqhub_get_spendable_1(v_account, v_asset) = 100;

	-- New consumption must block both legacy reinvestment and offramp.
	BEGIN
		PERFORM * FROM rfqhub_create_bundle_taker_from_aggregate_1(
			v_big, 1, TRUE, 100, 'BitcoinPrice', 1, v_expiry, 0, v_deadline);
		RAISE EXCEPTION 'legacy reinvestment unexpectedly accepted';
	EXCEPTION WHEN raise_exception THEN
		IF SQLERRM NOT LIKE '%already consumed%' THEN RAISE; END IF;
	END;
	BEGIN
		PERFORM * FROM rfqhub_mark_winner_as_offramped_1(v_account, TRUE, v_big);
		RAISE EXCEPTION 'offramp unexpectedly accepted';
	EXCEPTION WHEN raise_exception THEN
		IF SQLERRM NOT LIKE '%already consumed%' THEN RAISE; END IF;
	END;

	SELECT * INTO STRICT v_r FROM rfqhub_create_bundle_taker_1(
		v_account, v_asset, 40, 40, TRUE, 100, 'BitcoinPrice', 1, v_expiry, 0, v_deadline);
	ASSERT v_r.bundle_maker_structure IS NULL AND v_r.source_amt = 40;
	ASSERT v_r.amt = 40 AND rfqhub_get_spendable_1(v_account, v_asset) = 60;
	ASSERT v_r.source_maker_id IS NULL AND v_r.bundle_maker_structure IS NULL;
	ASSERT v_r.timestamplet_id IS NULL;
	ASSERT v_r.timestamplet_end_value IS NULL AND v_r.timestamplet_time IS NULL;
	ASSERT v_r.timestamplet_outcome IS NULL;

	v_source := pg_temp.seed_winner(v_account, v_asset, 20);
	v_rows := 0;
	FOR v_r IN SELECT * FROM rfqhub_create_bundle_taker_1(
		v_account, v_asset, 70, 90, TRUE, 100, 'BitcoinPrice', 1, v_expiry, 0, v_deadline)
	LOOP
		v_rows := v_rows + 1;
		v_bundle := v_r.bundle_taker_id;
		ASSERT v_r.amt = 80;
		IF v_rows = 1 THEN
			ASSERT (SELECT m.bundle_taker_id = v_source FROM rfqhub_bundles_maker_1 AS m WHERE m.id = v_r.source_maker_id) AND v_r.source_amt = 20;
		ELSE
			ASSERT v_r.bundle_maker_structure IS NULL AND v_r.source_amt = 60;
		END IF;
	END LOOP;
	ASSERT v_rows = 2;
	ASSERT (SELECT pot_funded_amt = 60 AND amt = 80 FROM rfqhub_bundles_taker_1 WHERE id = v_bundle);
	ASSERT rfqhub_get_spendable_1(v_account, v_asset) = 0;

	-- Minimum failure must leave an available source, bundle count and counter intact.
	v_source := pg_temp.seed_winner(v_account, v_asset, 5);
	SELECT count(*) INTO v_before FROM rfqhub_bundles_taker_1;
	SELECT count INTO v_count FROM rfqhub_bundles_taker_user_count_1 WHERE addr = v_account;
	BEGIN
		PERFORM * FROM rfqhub_create_bundle_taker_1(
			v_account, v_asset, 6, 10, TRUE, 100, 'BitcoinPrice', 1, v_expiry, 0, v_deadline);
		RAISE EXCEPTION 'minimum failure unexpectedly accepted';
	EXCEPTION WHEN raise_exception THEN
		IF SQLERRM NOT LIKE 'insufficient taker funding:%' THEN RAISE; END IF;
	END;
	ASSERT (SELECT count(*) = v_before FROM rfqhub_bundles_taker_1);
	ASSERT (SELECT count = v_count FROM rfqhub_bundles_taker_user_count_1 WHERE addr = v_account);
	ASSERT NOT EXISTS (SELECT 1 FROM rfqhub_bundles_taker_sources_1 WHERE source_taker_id = v_source);

	-- Legacy consumption also enters the shared ledger and never reserves pot.
	SELECT * INTO STRICT v_r FROM rfqhub_create_bundle_taker_from_aggregate_1(
		v_source, 2, TRUE, 100, 'BitcoinPrice', 1, v_expiry, 0, v_deadline);
	ASSERT v_r.amt = 2 AND v_r.amt_to_onramp_amt = 3;
	ASSERT (SELECT source = v_source AND pot_funded_amt = 0 FROM rfqhub_bundles_taker_1 WHERE id = v_r.bundle_taker_id);
	ASSERT (SELECT source_amt = 2 AND amt_to_onramp_amt = 3 FROM rfqhub_bundles_taker_sources_1 WHERE source_taker_id = v_source);
	ASSERT rfqhub_get_spendable_1(v_account, v_asset) = 0;

	-- Offramped, foreign-account/asset, unsigned and maker-winning rows are ineligible.
	v_source := pg_temp.seed_winner(v_account, v_asset, 9);
	PERFORM * FROM rfqhub_mark_winner_as_offramped_1(v_account, TRUE, v_source);
	PERFORM pg_temp.seed_winner('0x00000000000000000000000000000000000000a2', v_asset, 9);
	PERFORM pg_temp.seed_winner(v_account, '0x00000000000000000000000000000000000000b2', 9);
	v_source := pg_temp.seed_winner(v_account, v_asset, 9);
	UPDATE rfqhub_bundles_taker_1 SET structure = NULL WHERE id = v_source;
	v_source := pg_temp.seed_winner(v_account, v_asset, 9);
	UPDATE rfqhub_bundles_maker_1 SET structure = NULL WHERE bundle_taker_id = v_source;
	v_source := pg_temp.seed_winner(v_account, v_asset, 9);
	UPDATE rfqhub_bundles_maker_1 SET was_correct = TRUE, end_source_amt = 9 WHERE bundle_taker_id = v_source;
	UPDATE rfqhub_bundles_taker_1 SET end_source_amt = 0 WHERE id = v_source;
	v_source := pg_temp.seed_winner(v_account, v_asset, 9);
	UPDATE rfqhub_bundles_maker_1 SET timestamplet_id = NULL WHERE bundle_taker_id = v_source;
	v_source := pg_temp.seed_winner(v_account, v_asset, 9);
	UPDATE rfqhub_bundles_maker_1 SET was_chosen = FALSE WHERE bundle_taker_id = v_source;
	BEGIN
		PERFORM * FROM rfqhub_create_bundle_taker_1(
			v_account, v_asset, 1, 100, TRUE, 100, 'BitcoinPrice', 1, v_expiry, 0, v_deadline);
		RAISE EXCEPTION 'ineligible source unexpectedly accepted';
	EXCEPTION WHEN raise_exception THEN
		IF SQLERRM NOT LIKE 'insufficient taker funding:%' THEN RAISE; END IF;
	END;
END;
$$;
-- Repeated wins form another generation without reopening the original sources.
DO $$
DECLARE
	v_account ADDRESS := '0x00000000000000000000000000000000000000d1';
	v_asset ADDRESS := '0x00000000000000000000000000000000000000b1';
	v_parent INTEGER;
	v_child INTEGER;
	v_r RECORD;
BEGIN
	PERFORM pg_temp.seed_winner(v_account, v_asset, 10);
	PERFORM pg_temp.seed_winner(v_account, v_asset, 20);
	FOR v_r IN SELECT * FROM rfqhub_create_bundle_taker_1(
		v_account, v_asset, 30, 30, TRUE, 100, 'BitcoinPrice', 2,
		EXTRACT(EPOCH FROM TIMESTAMP '2026-01-01')::INTEGER, 0,
		EXTRACT(EPOCH FROM TIMESTAMP '2025-12-31')::INTEGER)
	LOOP
		v_parent := v_r.bundle_taker_id;
	END LOOP;
	PERFORM rfqhub_set_bundle_taker_structure_1(v_parent, '{"signed":"nested taker"}');
	INSERT INTO rfqhub_bundles_maker_1 (acc_addr, user_id, bundle_taker_id, ms_ts, amt, structure)
	VALUES ('0x00000000000000000000000000000000000000d2', 1, v_parent, 2, 15,
		'{"signed":"nested maker"}');
	PERFORM * FROM rfqhub_conclude_aggregate_1(v_parent, 200);
	SELECT * INTO STRICT v_r FROM rfqhub_create_bundle_taker_1(
		v_account, v_asset, 45, 45, TRUE, 100, 'BitcoinPrice', 3,
		EXTRACT(EPOCH FROM CURRENT_TIMESTAMP + INTERVAL '2 hours')::INTEGER, 0,
		EXTRACT(EPOCH FROM CURRENT_TIMESTAMP + INTERVAL '1 hour')::INTEGER);
	v_child := v_r.bundle_taker_id;
	ASSERT (SELECT m.bundle_taker_id = v_parent FROM rfqhub_bundles_maker_1 AS m WHERE m.id = v_r.source_maker_id) AND v_r.source_amt = 45;
	ASSERT v_r.bundle_maker_structure::JSONB = '{"signed":"nested maker"}'::JSONB;
	ASSERT rfqhub_get_spendable_1(v_account, v_asset) = 0;
	ASSERT (SELECT count(*) = 2 FROM rfqhub_bundles_taker_sources_1 WHERE bundle_taker_id = v_parent);
	ASSERT (SELECT count(*) = 1 FROM rfqhub_bundles_taker_sources_1 WHERE bundle_taker_id = v_child);
END;
$$;
ROLLBACK;
