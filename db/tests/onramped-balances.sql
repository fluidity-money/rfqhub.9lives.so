-- Run in a fresh session against a disposable migrated database:
-- psql -X -v ON_ERROR_STOP=1 -f db/tests/onramped-balances.sql
-- All fixtures roll back; sequence advances are not transactional.
BEGIN;
SET LOCAL plpgsql.check_asserts = on;
CREATE TEMP TABLE rfqhub_events_balance_changed (
	id BIGSERIAL PRIMARY KEY, account_addr ADDRESS, asset ADDRESS, amount BIGINT
);
-- The persistent view binds the real feed, so shadow it along with the table.
CREATE TEMP VIEW rfqhub_onramped_balances_1 AS
SELECT account_addr, asset, SUM(amount) AS amount
FROM rfqhub_events_balance_changed
GROUP BY account_addr, asset;

DO $$
DECLARE
	v_account ADDRESS := '0x00000000000000000000000000000000000000d1';
	v_other ADDRESS := '0x00000000000000000000000000000000000000d2';
	v_asset ADDRESS := '0x00000000000000000000000000000000000000d3';
	v_other_asset ADDRESS := '0x00000000000000000000000000000000000000d4';
BEGIN
	ASSERT rfqhub_get_spendable_1(v_account, v_asset) = 0, 'no events means zero';
	ASSERT rfqhub_get_available_balance_1(v_account, v_asset) = 0;
	INSERT INTO rfqhub_events_balance_changed (account_addr, asset, amount)
	VALUES (v_account, v_asset, 20), (v_account, v_asset, 20),
		(v_account, v_asset, 7), (v_other, v_asset, 200),
		(v_account, v_other_asset, 300);
	ASSERT (SELECT amount = 47 AND pg_typeof(amount) = 'numeric'::regtype
		FROM rfqhub_onramped_balances_1 WHERE account_addr = v_account AND asset = v_asset),
		'sum every credit including equal-valued deposits';
	ASSERT rfqhub_get_spendable_1(v_account, v_asset) = 47;
	ASSERT rfqhub_get_available_balance_1(v_account, v_asset) = 47;
	ASSERT rfqhub_get_spendable_1(v_other, v_asset) = 200, 'account isolation';
	ASSERT rfqhub_get_spendable_1(v_account, v_other_asset) = 300, 'asset isolation';
	ASSERT rfqhub_get_spendable_1(v_other, v_other_asset) = 0, 'missing pair means zero';

	-- Keep the grouped total NUMERIC; fail closed at the BIGINT consumer rather
	-- than wrapping or silently truncating an out-of-range total.
	INSERT INTO rfqhub_events_balance_changed (account_addr, asset, amount)
	VALUES (v_other, v_other_asset, 9223372036854775807), (v_other, v_other_asset, 1);
	ASSERT (SELECT amount = 9223372036854775808::NUMERIC
		FROM rfqhub_onramped_balances_1 WHERE account_addr = v_other AND asset = v_other_asset);
	BEGIN
		PERFORM rfqhub_get_spendable_1(v_other, v_other_asset);
		RAISE EXCEPTION 'out-of-range spendable total unexpectedly accepted';
	EXCEPTION WHEN numeric_value_out_of_range THEN NULL;
	END;
	BEGIN
		PERFORM rfqhub_get_available_balance_1(v_other, v_other_asset);
		RAISE EXCEPTION 'out-of-range available total unexpectedly accepted';
	EXCEPTION WHEN numeric_value_out_of_range THEN NULL;
	END;
	RAISE NOTICE 'PASS: additive credits, equal deposits, isolation, no events, BIGINT overflow';
END;
$$;
ROLLBACK;
