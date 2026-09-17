-- migrate:up

-- BalanceChanged is externally managed, but RFQHub needs the latest absolute
-- balance for one account/asset pair on every spendable-balance lookup.
CREATE INDEX rfqhub_events_balance_changed_latest_idx
	ON rfqhub_events_balance_changed (account_addr, asset, id DESC)
	INCLUDE (amount);

CREATE TABLE rfqhub_bundles_taker_user_count_1 (
	id SERIAL PRIMARY KEY,
	addr ADDRESS NOT NULL UNIQUE,
	count INTEGER NOT NULL DEFAULT 0 CHECK (count >= 0)
);

CREATE TYPE RFQHUB_OUTCOME AS ENUM ('BitcoinPrice');

-- A timestamplet is immutable once observed. Every aggregate for an outcome
-- and expiry shares the same authoritative value.
CREATE TABLE rfqhub_timestamplets_1 (
	id SERIAL PRIMARY KEY,
	created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
	timestamplet_time TIMESTAMP NOT NULL,
	timestamplet_outcome RFQHUB_OUTCOME NOT NULL,
	timestamplet_val BIGINT NOT NULL CHECK (timestamplet_val >= 0),
	UNIQUE (timestamplet_outcome, timestamplet_time)
);

CREATE FUNCTION rfqhub_protect_immutable_timestamplet_1() RETURNS trigger
LANGUAGE plpgsql
AS $$
BEGIN
	RAISE EXCEPTION 'timestamplets are immutable and cannot be modified';
END;
$$;

CREATE TRIGGER rfqhub_timestamplet_no_update_1
	BEFORE UPDATE ON rfqhub_timestamplets_1
	FOR EACH ROW EXECUTE FUNCTION rfqhub_protect_immutable_timestamplet_1();

CREATE TRIGGER rfqhub_timestamplet_no_delete_1
	BEFORE DELETE ON rfqhub_timestamplets_1
	FOR EACH ROW EXECUTE FUNCTION rfqhub_protect_immutable_timestamplet_1();

-- A BundleTaker can take its liquidity directly from the account's onchain pot
-- (source is NULL), or from the taker result of an earlier aggregate (source is
-- the earlier taker row).
CREATE TABLE rfqhub_bundles_taker_1 (
	id SERIAL PRIMARY KEY,
	created_by TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
	updated_by TIMESTAMP,
	acc_addr ADDRESS NOT NULL,
	user_id INTEGER NOT NULL,
	source INTEGER REFERENCES rfqhub_bundles_taker_1 (id),
	asset ADDRESS NOT NULL,
	amt BIGINT NOT NULL CHECK (amt >= 0),
	is_up BOOLEAN NOT NULL,
	price_target BIGINT NOT NULL CHECK (price_target >= 0),
	outcome RFQHUB_OUTCOME NOT NULL,
	maker_id INTEGER,
	ms_ts BIGINT NOT NULL,
	deadline TIMESTAMP NOT NULL,
	expiry TIMESTAMP NOT NULL,
	user_sig BYTES64 UNIQUE,
	was_offramped BOOLEAN NOT NULL DEFAULT FALSE,
	end_source_amt BIGINT CHECK (end_source_amt >= 0),
	CHECK (deadline <= expiry),
	UNIQUE (acc_addr, user_id)
);

-- An aggregate result is all-or-nothing. A previous taker result cannot be the
-- source of two later bundles.
CREATE UNIQUE INDEX rfqhub_bundles_taker_source_uq
	ON rfqhub_bundles_taker_1 (source)
	WHERE source IS NOT NULL;

CREATE INDEX rfqhub_bundles_taker_open_balance_idx
	ON rfqhub_bundles_taker_1 (acc_addr, asset)
	INCLUDE (id, amt, deadline)
	WHERE source IS NULL
		AND end_source_amt IS NULL
		AND was_offramped = FALSE;

CREATE TABLE rfqhub_bundles_maker_user_count_1 (
	id SERIAL PRIMARY KEY,
	addr ADDRESS NOT NULL UNIQUE,
	count INTEGER NOT NULL DEFAULT 0 CHECK (count >= 0)
);

-- BundleMaker has the same OnrampingSource alternatives as BundleTaker. For an
-- aggregate source, source identifies the selected maker row from the earlier
-- aggregate; its account and asset are derived rather than accepted from callers.
CREATE TABLE rfqhub_bundles_maker_1 (
	id SERIAL PRIMARY KEY,
	created_by TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
	updated_by TIMESTAMP,
	acc_addr ADDRESS NOT NULL,
	user_id INTEGER NOT NULL,
	source INTEGER REFERENCES rfqhub_bundles_maker_1 (id),
	bundle_taker_id INTEGER NOT NULL REFERENCES rfqhub_bundles_taker_1 (id),
	ms_ts BIGINT NOT NULL,
	amt BIGINT NOT NULL CHECK (amt >= 0),
	was_correct BOOLEAN,
	was_chosen BOOLEAN,
	timestamplet_id INTEGER REFERENCES rfqhub_timestamplets_1 (id),
	user_sig BYTES64 UNIQUE,
	server_sig BYTES64,
	was_offramped BOOLEAN NOT NULL DEFAULT FALSE,
	end_source_amt BIGINT CHECK (end_source_amt >= 0),
	UNIQUE (acc_addr, user_id),
	CHECK (timestamplet_id IS NULL OR server_sig IS NOT NULL)
);

CREATE UNIQUE INDEX rfqhub_bundles_maker_source_uq
	ON rfqhub_bundles_maker_1 (source)
	WHERE source IS NOT NULL;

CREATE INDEX rfqhub_bundles_maker_taker_idx
	ON rfqhub_bundles_maker_1 (bundle_taker_id);

CREATE INDEX rfqhub_bundles_maker_open_balance_idx
	ON rfqhub_bundles_maker_1 (acc_addr, bundle_taker_id)
	INCLUDE (amt)
	WHERE source IS NULL
		AND was_chosen IS DISTINCT FROM FALSE
		AND end_source_amt IS NULL
		AND was_offramped = FALSE;

ALTER TABLE rfqhub_bundles_taker_1
	ADD CONSTRAINT rfqhub_bundles_taker_maker_fk
	FOREIGN KEY (maker_id) REFERENCES rfqhub_bundles_maker_1 (id);

-- BundleTakers remain open for maker selection until their supplied deadline.
-- Unsigned takers are excluded because they cannot be submitted to the contract,
-- and selecting a maker closes the auction immediately.
CREATE VIEW rfqhub_bundles_taker_ongoing_1 AS
SELECT
	t.*,
	t.deadline AS auction_deadline
FROM rfqhub_bundles_taker_1 AS t
WHERE t.user_sig IS NOT NULL
	AND t.maker_id IS NULL
	AND t.deadline > CURRENT_TIMESTAMP;

CREATE FUNCTION rfqhub_inspect_bundle_taker_1 (
	p_acc_addr ADDRESS,
	p_bundle_taker_id INTEGER
)
RETURNS TABLE (
	bundle_taker rfqhub_bundles_taker_1,
	bundle_maker rfqhub_bundles_maker_1
)
LANGUAGE plpgsql
STABLE
AS $$
DECLARE
	v_bundle_taker rfqhub_bundles_taker_1%ROWTYPE;
BEGIN
	SELECT t.* INTO v_bundle_taker
	FROM rfqhub_bundles_taker_1 AS t
	WHERE t.id = p_bundle_taker_id;

	IF NOT FOUND THEN
		RETURN;
	END IF;
	IF v_bundle_taker.acc_addr IS DISTINCT FROM p_acc_addr THEN
		RAISE EXCEPTION 'account address does not match bundle taker %', p_bundle_taker_id;
	END IF;

	RETURN QUERY
	SELECT v_bundle_taker, m
	FROM rfqhub_bundles_maker_1 AS m
	WHERE m.id = v_bundle_taker.maker_id
	UNION ALL
	SELECT v_bundle_taker, NULL::rfqhub_bundles_maker_1
	WHERE v_bundle_taker.maker_id IS NULL;
END;
$$;

-- Return the unreserved part of the latest BalanceChanged amount. Aggregate
-- results are not included here until a result is partially returned to the pot
-- and recorded by a later BalanceChanged event.
CREATE FUNCTION rfqhub_get_spendable_1 (
	p_acc_addr ADDRESS,
	p_asset ADDRESS
)
RETURNS BIGINT
LANGUAGE plpgsql
STABLE
AS $$
DECLARE
	v_balance BIGINT;
	v_taker_reserved BIGINT;
	v_maker_reserved BIGINT;
BEGIN
	SELECT COALESCE((
		SELECT e.amount
		FROM rfqhub_events_balance_changed AS e
		WHERE e.account_addr = p_acc_addr
			AND e.asset = p_asset
		ORDER BY e.id DESC
		LIMIT 1
	), 0)
	INTO v_balance;

	SELECT COALESCE(SUM(t.amt), 0)
	INTO v_taker_reserved
	FROM rfqhub_bundles_taker_1 AS t
	WHERE t.acc_addr = p_acc_addr
		AND t.asset = p_asset
		AND t.source IS NULL
		AND t.end_source_amt IS NULL
		AND t.was_offramped = FALSE
		-- An unmatched taker only reserves its pot balance while its auction is
		-- open. If at least one maker exists, keep the reservation because a
		-- maker is expected to be selected automatically when the auction ends.
		AND (
			t.deadline > CURRENT_TIMESTAMP
			OR EXISTS (
				SELECT 1
				FROM rfqhub_bundles_maker_1 AS candidate
				WHERE candidate.bundle_taker_id = t.id
			)
		);

	SELECT COALESCE(SUM(m.amt), 0)
	INTO v_maker_reserved
	FROM rfqhub_bundles_maker_1 AS m
	JOIN rfqhub_bundles_taker_1 AS t ON t.id = m.bundle_taker_id
	WHERE m.acc_addr = p_acc_addr
		AND t.asset = p_asset
		AND m.source IS NULL
		AND m.was_chosen IS DISTINCT FROM FALSE
		AND m.end_source_amt IS NULL
		AND m.was_offramped = FALSE;

	RETURN GREATEST(v_balance - v_taker_reserved - v_maker_reserved, 0);
END;
$$;

CREATE FUNCTION rfqhub_get_or_create_timestamplet_1 (
	p_timestamplet_time TIMESTAMP,
	p_timestamplet_outcome RFQHUB_OUTCOME,
	p_timestamplet_val BIGINT
)
RETURNS INTEGER
LANGUAGE plpgsql
AS $$
DECLARE
	v_timestamplet_id INTEGER;
	v_existing_val BIGINT;
BEGIN
	IF p_timestamplet_val < 0 THEN
		RAISE EXCEPTION 'timestamplet value cannot be negative';
	END IF;

	INSERT INTO rfqhub_timestamplets_1 (
		timestamplet_time,
		timestamplet_outcome,
		timestamplet_val
	)
	VALUES (
		p_timestamplet_time,
		p_timestamplet_outcome,
		p_timestamplet_val
	)
	ON CONFLICT (timestamplet_outcome, timestamplet_time) DO NOTHING
	RETURNING id INTO v_timestamplet_id;

	IF v_timestamplet_id IS NOT NULL THEN
		RETURN v_timestamplet_id;
	END IF;

	SELECT t.id, t.timestamplet_val
	INTO STRICT v_timestamplet_id, v_existing_val
	FROM rfqhub_timestamplets_1 AS t
	WHERE t.timestamplet_outcome = p_timestamplet_outcome
		AND t.timestamplet_time = p_timestamplet_time;

	IF v_existing_val <> p_timestamplet_val THEN
		RAISE EXCEPTION
			'conflicting timestamplet for outcome %, time %: existing value %, supplied value %',
			p_timestamplet_outcome,
			p_timestamplet_time,
			v_existing_val,
			p_timestamplet_val;
	END IF;

	RETURN v_timestamplet_id;
END;
$$;

CREATE FUNCTION rfqhub_create_bundle_taker_onramping_1 (
	p_acc_addr ADDRESS,
	p_asset ADDRESS,
	p_min_amt BIGINT,
	p_max_amt BIGINT,
	p_is_up BOOLEAN,
	p_price_target BIGINT,
	p_outcome RFQHUB_OUTCOME,
	p_ms_ts BIGINT,
	p_expiry INTEGER,
	p_deadline INTEGER
)
RETURNS TABLE (
	bundle_taker_id INTEGER,
	amt BIGINT,
	leftover_bal BIGINT
)
LANGUAGE plpgsql
AS $$
DECLARE
	v_available BIGINT;
	v_amt BIGINT;
	v_user_id INTEGER;
	v_bundle_taker_id INTEGER;
	v_expiry TIMESTAMP;
	v_deadline TIMESTAMP;
BEGIN
	IF p_min_amt <= 0 OR p_max_amt < p_min_amt THEN
		RAISE EXCEPTION 'invalid taker amount range: minimum %, maximum %', p_min_amt, p_max_amt;
	END IF;
	v_expiry := TO_TIMESTAMP(p_expiry) AT TIME ZONE 'UTC';
	IF p_deadline IS NULL THEN
		RAISE EXCEPTION 'taker deadline cannot be null';
	END IF;
	v_deadline := TO_TIMESTAMP(p_deadline) AT TIME ZONE 'UTC';
	IF v_deadline > v_expiry THEN
		RAISE EXCEPTION 'taker deadline % cannot be after expiry %', v_deadline, v_expiry;
	END IF;

	v_available := rfqhub_get_spendable_1(p_acc_addr, p_asset);
	v_amt := LEAST(p_max_amt, v_available);
	IF v_amt < p_min_amt THEN
		RAISE EXCEPTION 'insufficient spendable balance: available %, minimum %', v_available, p_min_amt;
	END IF;

	INSERT INTO rfqhub_bundles_taker_user_count_1 AS c (addr, count)
	VALUES (p_acc_addr, 1)
	ON CONFLICT (addr) DO UPDATE SET count = c.count + 1
	RETURNING count INTO v_user_id;

	INSERT INTO rfqhub_bundles_taker_1 (
		acc_addr, user_id, source, asset, amt, is_up, price_target,
		outcome, ms_ts, deadline, expiry
	)
	VALUES (
		p_acc_addr, v_user_id, NULL, p_asset, v_amt, p_is_up,
		p_price_target, p_outcome, p_ms_ts, v_deadline, v_expiry
	)
	RETURNING id INTO v_bundle_taker_id;

	RETURN QUERY SELECT v_bundle_taker_id, v_amt, v_available - v_amt;
END;
$$;

-- Onramped taker bundles are created before the final amount is known, so the
-- user signature is attached after creation once that amount has been signed.
-- Replaying the same signature is harmless; replacing it is not allowed.
CREATE FUNCTION rfqhub_set_bundle_taker_user_sig_1 (
	p_bundle_taker_id INTEGER,
	p_user_sig BYTES64
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
DECLARE
	v_user_sig BYTES64;
BEGIN
	IF p_user_sig IS NULL THEN
		RAISE EXCEPTION 'taker user signature cannot be null';
	END IF;

	SELECT t.user_sig INTO STRICT v_user_sig
	FROM rfqhub_bundles_taker_1 AS t
	WHERE t.id = p_bundle_taker_id
	FOR UPDATE;

	IF v_user_sig IS NOT NULL AND v_user_sig <> p_user_sig THEN
		RAISE EXCEPTION 'taker % user signature is already set', p_bundle_taker_id;
	END IF;

	IF v_user_sig IS NULL THEN
		UPDATE rfqhub_bundles_taker_1
		SET user_sig = p_user_sig,
			updated_by = CURRENT_TIMESTAMP
		WHERE id = p_bundle_taker_id;
	END IF;
END;
$$;

CREATE FUNCTION rfqhub_create_bundle_taker_from_aggregate_1 (
	p_source_taker_id INTEGER,
	p_amt_to_take BIGINT,
	p_is_up BOOLEAN,
	p_price_target BIGINT,
	p_outcome RFQHUB_OUTCOME,
	p_ms_ts BIGINT,
	p_expiry INTEGER,
	p_deadline INTEGER,
	p_user_sig BYTES64
)
RETURNS TABLE (
	bundle_taker_id INTEGER,
	amt BIGINT,
	amt_to_onramp_amt BIGINT
)
LANGUAGE plpgsql
AS $$
DECLARE
	v_source rfqhub_bundles_taker_1%ROWTYPE;
	v_user_id INTEGER;
	v_bundle_taker_id INTEGER;
	v_remainder BIGINT;
	v_expiry TIMESTAMP;
	v_deadline TIMESTAMP;
BEGIN
	v_expiry := TO_TIMESTAMP(p_expiry) AT TIME ZONE 'UTC';
	IF p_deadline IS NULL THEN
		RAISE EXCEPTION 'taker deadline cannot be null';
	END IF;
	v_deadline := TO_TIMESTAMP(p_deadline) AT TIME ZONE 'UTC';
	IF v_deadline > v_expiry THEN
		RAISE EXCEPTION 'taker deadline % cannot be after expiry %', v_deadline, v_expiry;
	END IF;

	SELECT t.* INTO STRICT v_source
	FROM rfqhub_bundles_taker_1 AS t
	WHERE t.id = p_source_taker_id
	FOR UPDATE;

	IF v_source.end_source_amt IS NULL THEN
		RAISE EXCEPTION 'source taker % has not been resolved', p_source_taker_id;
	END IF;
	IF v_source.end_source_amt = 0 THEN
		RAISE EXCEPTION 'source taker % has no winning amount', p_source_taker_id;
	END IF;
	IF v_source.was_offramped THEN
		RAISE EXCEPTION 'source taker % was already offramped', p_source_taker_id;
	END IF;
	IF p_amt_to_take <= 0 OR p_amt_to_take > v_source.end_source_amt THEN
		RAISE EXCEPTION 'invalid amount % for source taker balance %', p_amt_to_take, v_source.end_source_amt;
	END IF;
	IF EXISTS (
		SELECT 1 FROM rfqhub_bundles_taker_1 AS t
		WHERE t.source = p_source_taker_id
	) THEN
		RAISE EXCEPTION 'source taker % was already consumed', p_source_taker_id;
	END IF;

	v_remainder := v_source.end_source_amt - p_amt_to_take;
	-- Any leftover elected not to reinvest is returned to the pot by the
	-- contract, which emits an event that the graph projects. We do not apply
	-- a local delta to the absolute onramped balance; it is purely
	-- informational here for the caller.

	INSERT INTO rfqhub_bundles_taker_user_count_1 AS c (addr, count)
	VALUES (v_source.acc_addr, 1)
	ON CONFLICT (addr) DO UPDATE SET count = c.count + 1
	RETURNING count INTO v_user_id;

	INSERT INTO rfqhub_bundles_taker_1 (
		acc_addr, user_id, source, asset, amt, is_up, price_target,
		outcome, ms_ts, deadline, expiry, user_sig
	)
	VALUES (
		v_source.acc_addr, v_user_id, p_source_taker_id, v_source.asset,
		p_amt_to_take, p_is_up, p_price_target, p_outcome, p_ms_ts,
		v_deadline, v_expiry, p_user_sig
	)
	RETURNING id INTO v_bundle_taker_id;

	RETURN QUERY SELECT v_bundle_taker_id, p_amt_to_take, v_remainder;
END;
$$;

CREATE FUNCTION rfqhub_create_bundle_maker_onramping_1 (
	p_acc_addr ADDRESS,
	p_bundle_taker_id INTEGER,
	p_min_amt BIGINT,
	p_max_amt BIGINT,
	p_ms_ts BIGINT
)
RETURNS TABLE (
	bundle_maker_id INTEGER,
	maker_amt BIGINT,
	taker_source INTEGER,
	taker_acc_addr ADDRESS,
	taker_asset ADDRESS,
	taker_amt BIGINT,
	taker_expiry INTEGER,
	taker_ms_ts BIGINT,
	taker_is_up BOOLEAN,
	taker_price_target BIGINT,
	taker_outcome RFQHUB_OUTCOME,
	taker_user_sig BYTES64
)
LANGUAGE plpgsql
AS $$
DECLARE
	v_taker rfqhub_bundles_taker_1%ROWTYPE;
	v_available BIGINT;
	v_amt BIGINT;
	v_user_id INTEGER;
	v_bundle_maker_id INTEGER;
BEGIN
	IF p_min_amt <= 0 OR p_max_amt < p_min_amt THEN
		RAISE EXCEPTION 'invalid maker amount range: minimum %, maximum %', p_min_amt, p_max_amt;
	END IF;

	SELECT t.* INTO STRICT v_taker
	FROM rfqhub_bundles_taker_1 AS t
	WHERE t.id = p_bundle_taker_id
	FOR SHARE;
	IF v_taker.maker_id IS NOT NULL THEN
		RAISE EXCEPTION 'taker % already selected maker %', p_bundle_taker_id, v_taker.maker_id;
	END IF;
	IF v_taker.deadline <= CURRENT_TIMESTAMP THEN
		RAISE EXCEPTION 'taker % deadline has passed', p_bundle_taker_id;
	END IF;
	v_available := rfqhub_get_spendable_1(p_acc_addr, v_taker.asset);
	v_amt := LEAST(p_max_amt, v_available);
	IF v_amt < p_min_amt THEN
		RAISE EXCEPTION 'insufficient spendable balance: available %, minimum %', v_available, p_min_amt;
	END IF;
	INSERT INTO rfqhub_bundles_maker_user_count_1 AS c (addr, count)
	VALUES (p_acc_addr, 1)
	ON CONFLICT (addr) DO UPDATE SET count = c.count + 1
	RETURNING count INTO v_user_id;

	INSERT INTO rfqhub_bundles_maker_1 (
		acc_addr, user_id, source, bundle_taker_id, ms_ts, amt
	)
	VALUES (
		p_acc_addr, v_user_id, NULL, p_bundle_taker_id, p_ms_ts, v_amt
	)
	RETURNING id INTO v_bundle_maker_id;

	RETURN QUERY
	SELECT
		v_bundle_maker_id,
		v_amt,
		v_taker.source,
		v_taker.acc_addr,
		v_taker.asset,
		v_taker.amt,
		EXTRACT(EPOCH FROM v_taker.expiry)::INTEGER,
		v_taker.ms_ts,
		v_taker.is_up,
		v_taker.price_target,
		v_taker.outcome,
		v_taker.user_sig;
END;
$$;

-- Onramped maker bundles are created before their final amount is known, so
-- the user signature is attached after creation once that amount has been
-- signed. Replaying the same signature is harmless; replacing it is not.
CREATE FUNCTION rfqhub_set_bundle_maker_user_sig_1 (
	p_bundle_maker_id INTEGER,
	p_user_sig BYTES64
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
DECLARE
	v_user_sig BYTES64;
BEGIN
	IF p_user_sig IS NULL THEN
		RAISE EXCEPTION 'maker user signature cannot be null';
	END IF;

	SELECT m.user_sig INTO STRICT v_user_sig
	FROM rfqhub_bundles_maker_1 AS m
	WHERE m.id = p_bundle_maker_id
	FOR UPDATE;

	IF v_user_sig IS NOT NULL AND v_user_sig <> p_user_sig THEN
		RAISE EXCEPTION 'maker % user signature is already set', p_bundle_maker_id;
	END IF;

	IF v_user_sig IS NULL THEN
		UPDATE rfqhub_bundles_maker_1
		SET user_sig = p_user_sig,
			updated_by = CURRENT_TIMESTAMP
		WHERE id = p_bundle_maker_id;
	END IF;
END;
$$;

CREATE FUNCTION rfqhub_create_bundle_maker_from_aggregate_1 (
	p_source_maker_id INTEGER,
	p_bundle_taker_id INTEGER,
	p_amt_to_take BIGINT,
	p_ms_ts BIGINT,
	p_user_sig BYTES64
)
RETURNS TABLE (
	bundle_maker_id INTEGER,
	amt BIGINT,
	amt_to_onramp_amt BIGINT
)
LANGUAGE plpgsql
AS $$
DECLARE
	v_source rfqhub_bundles_maker_1%ROWTYPE;
	v_source_taker rfqhub_bundles_taker_1%ROWTYPE;
	v_target_taker rfqhub_bundles_taker_1%ROWTYPE;
	v_user_id INTEGER;
	v_bundle_maker_id INTEGER;
	v_remainder BIGINT;
BEGIN
	SELECT m.* INTO STRICT v_source
	FROM rfqhub_bundles_maker_1 AS m
	WHERE m.id = p_source_maker_id
	FOR UPDATE;

	SELECT t.* INTO STRICT v_source_taker
	FROM rfqhub_bundles_taker_1 AS t
	WHERE t.id = v_source.bundle_taker_id;

	SELECT t.* INTO STRICT v_target_taker
	FROM rfqhub_bundles_taker_1 AS t
	WHERE t.id = p_bundle_taker_id
	FOR SHARE;

	IF v_target_taker.maker_id IS NOT NULL THEN
		RAISE EXCEPTION 'taker % already selected maker %', p_bundle_taker_id, v_target_taker.maker_id;
	END IF;
	IF v_target_taker.deadline <= CURRENT_TIMESTAMP THEN
		RAISE EXCEPTION 'taker % deadline has passed', p_bundle_taker_id;
	END IF;
	IF v_source.end_source_amt IS NULL THEN
		RAISE EXCEPTION 'source maker % has not been resolved', p_source_maker_id;
	END IF;
	IF v_source.end_source_amt = 0 THEN
		RAISE EXCEPTION 'source maker % has no winning amount', p_source_maker_id;
	END IF;
	IF v_source.was_offramped THEN
		RAISE EXCEPTION 'source maker % was already offramped', p_source_maker_id;
	END IF;
	IF v_source_taker.asset <> v_target_taker.asset THEN
		RAISE EXCEPTION 'source and target maker assets differ';
	END IF;
	IF p_amt_to_take <= 0 OR p_amt_to_take > v_source.end_source_amt THEN
		RAISE EXCEPTION 'invalid amount % for source maker balance %', p_amt_to_take, v_source.end_source_amt;
	END IF;
	IF EXISTS (
		SELECT 1 FROM rfqhub_bundles_maker_1 AS m
		WHERE m.source = p_source_maker_id
	) THEN
		RAISE EXCEPTION 'source maker % was already consumed', p_source_maker_id;
	END IF;

	v_remainder := v_source.end_source_amt - p_amt_to_take;
	-- Any leftover elected not to reinvest is returned to the pot by the
	-- contract, which emits an event that the graph projects. We do not apply
	-- a local delta to the absolute onramped balance; it is purely
	-- informational here for the caller.

	INSERT INTO rfqhub_bundles_maker_user_count_1 AS c (addr, count)
	VALUES (v_source.acc_addr, 1)
	ON CONFLICT (addr) DO UPDATE SET count = c.count + 1
	RETURNING count INTO v_user_id;

	INSERT INTO rfqhub_bundles_maker_1 (
		acc_addr, user_id, source, bundle_taker_id, ms_ts, amt, user_sig
	)
	VALUES (
		v_source.acc_addr, v_user_id, p_source_maker_id, p_bundle_taker_id,
		p_ms_ts, p_amt_to_take, p_user_sig
	)
	RETURNING id INTO v_bundle_maker_id;

	RETURN QUERY SELECT v_bundle_maker_id, p_amt_to_take, v_remainder;
END;
$$;

-- The taker is derived from the maker, preventing callers from supplying an
-- inconsistent pair.
CREATE FUNCTION rfqhub_pick_maker_1 (
	p_bundle_maker_id INTEGER
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
DECLARE
	v_maker rfqhub_bundles_maker_1%ROWTYPE;
	v_existing_maker_id INTEGER;
BEGIN
	SELECT m.* INTO STRICT v_maker
	FROM rfqhub_bundles_maker_1 AS m
	WHERE m.id = p_bundle_maker_id
	FOR UPDATE;

	SELECT t.maker_id INTO v_existing_maker_id
	FROM rfqhub_bundles_taker_1 AS t
	WHERE t.id = v_maker.bundle_taker_id
	FOR UPDATE;

	IF v_existing_maker_id IS NOT NULL AND v_existing_maker_id <> p_bundle_maker_id THEN
		RAISE EXCEPTION 'taker % already selected maker %', v_maker.bundle_taker_id, v_existing_maker_id;
	END IF;

	UPDATE rfqhub_bundles_maker_1 AS m
	SET was_chosen = (m.id = p_bundle_maker_id),
		updated_by = CURRENT_TIMESTAMP
	WHERE m.bundle_taker_id = v_maker.bundle_taker_id;

	UPDATE rfqhub_bundles_taker_1 AS t
	SET maker_id = p_bundle_maker_id,
		updated_by = CURRENT_TIMESTAMP
	WHERE t.id = v_maker.bundle_taker_id;
END;
$$;

-- Resolve the selected pair. Taker identity, expiry and outcome are looked up;
-- the timestamplet is created or reused atomically.
CREATE FUNCTION rfqhub_resolve_aggregate_1 (
	p_bundle_maker_id INTEGER,
	p_timestamplet_val BIGINT,
	p_server_sig BYTES64
)
RETURNS TABLE (
	bundle_taker_id INTEGER,
	bundle_maker_id INTEGER,
	timestamplet_id INTEGER,
	winner_acc_addr ADDRESS,
	taker_was_correct BOOLEAN,
	taker_end_source_amt BIGINT,
	maker_end_source_amt BIGINT
)
LANGUAGE plpgsql
AS $$
DECLARE
	v_maker rfqhub_bundles_maker_1%ROWTYPE;
	v_taker rfqhub_bundles_taker_1%ROWTYPE;
	v_timestamplet_id INTEGER;
	v_taker_correct BOOLEAN;
	v_total BIGINT;
	v_taker_end BIGINT;
	v_maker_end BIGINT;
	v_winner ADDRESS;
	v_existing_val BIGINT;
BEGIN
	IF p_server_sig IS NULL THEN
		RAISE EXCEPTION 'server signature is required to resolve an aggregate';
	END IF;

	SELECT m.* INTO STRICT v_maker
	FROM rfqhub_bundles_maker_1 AS m
	WHERE m.id = p_bundle_maker_id
	FOR UPDATE;

	SELECT t.* INTO STRICT v_taker
	FROM rfqhub_bundles_taker_1 AS t
	WHERE t.id = v_maker.bundle_taker_id
	FOR UPDATE;

	IF v_taker.maker_id IS DISTINCT FROM p_bundle_maker_id OR v_maker.was_chosen IS DISTINCT FROM TRUE THEN
		RAISE EXCEPTION 'maker % is not selected for taker %', p_bundle_maker_id, v_taker.id;
	END IF;

	IF v_maker.timestamplet_id IS NOT NULL THEN
		SELECT ts.timestamplet_val INTO v_existing_val
		FROM rfqhub_timestamplets_1 AS ts
		WHERE ts.id = v_maker.timestamplet_id;
		IF v_existing_val <> p_timestamplet_val OR v_maker.server_sig IS DISTINCT FROM p_server_sig THEN
			RAISE EXCEPTION 'aggregate for maker % was already resolved differently', p_bundle_maker_id;
		END IF;
		RETURN QUERY SELECT
			v_taker.id,
			v_maker.id,
			v_maker.timestamplet_id,
			CASE WHEN v_maker.was_correct THEN v_maker.acc_addr ELSE v_taker.acc_addr END,
			NOT v_maker.was_correct,
			v_taker.end_source_amt,
			v_maker.end_source_amt;
		RETURN;
	END IF;

	v_timestamplet_id := rfqhub_get_or_create_timestamplet_1(
		v_taker.expiry,
		v_taker.outcome,
		p_timestamplet_val
	);
	v_taker_correct := (p_timestamplet_val > v_taker.price_target AND v_taker.is_up)
		OR (p_timestamplet_val <= v_taker.price_target AND NOT v_taker.is_up);
	v_total := v_taker.amt + v_maker.amt;
	v_taker_end := CASE WHEN v_taker_correct THEN v_total ELSE 0 END;
	v_maker_end := CASE WHEN v_taker_correct THEN 0 ELSE v_total END;
	v_winner := CASE WHEN v_taker_correct THEN v_taker.acc_addr ELSE v_maker.acc_addr END;

	UPDATE rfqhub_bundles_taker_1 AS t
	SET end_source_amt = v_taker_end,
		updated_by = CURRENT_TIMESTAMP
	WHERE t.id = v_taker.id;

	UPDATE rfqhub_bundles_maker_1 AS m
	SET was_correct = NOT v_taker_correct,
		timestamplet_id = v_timestamplet_id,
		server_sig = p_server_sig,
		end_source_amt = v_maker_end,
		updated_by = CURRENT_TIMESTAMP
	WHERE m.id = v_maker.id;

	RETURN QUERY SELECT
		v_taker.id,
		v_maker.id,
		v_timestamplet_id,
		v_winner,
		v_taker_correct,
		v_taker_end,
		v_maker_end;
END;
$$;

-- The selected taker is derived from the maker. Calling this twice is
-- idempotent, but unresolved or already-consumed results are rejected.
CREATE FUNCTION rfqhub_mark_as_offramped_1 (
	p_bundle_maker_id INTEGER
)
RETURNS TABLE (
	taker_end_source_amt BIGINT,
	maker_end_source_amt BIGINT
)
LANGUAGE plpgsql
AS $$
DECLARE
	v_maker rfqhub_bundles_maker_1%ROWTYPE;
	v_taker rfqhub_bundles_taker_1%ROWTYPE;
BEGIN
	SELECT m.* INTO STRICT v_maker
	FROM rfqhub_bundles_maker_1 AS m
	WHERE m.id = p_bundle_maker_id
	FOR UPDATE;

	SELECT t.* INTO STRICT v_taker
	FROM rfqhub_bundles_taker_1 AS t
	WHERE t.id = v_maker.bundle_taker_id
	FOR UPDATE;

	IF v_taker.maker_id IS DISTINCT FROM p_bundle_maker_id OR v_maker.was_chosen IS DISTINCT FROM TRUE THEN
		RAISE EXCEPTION 'maker % is not selected for taker %', p_bundle_maker_id, v_taker.id;
	END IF;
	IF v_taker.end_source_amt IS NULL OR v_maker.end_source_amt IS NULL THEN
		RAISE EXCEPTION 'aggregate for maker % has not been resolved', p_bundle_maker_id;
	END IF;
	IF EXISTS (SELECT 1 FROM rfqhub_bundles_taker_1 AS t WHERE t.source = v_taker.id)
		OR EXISTS (SELECT 1 FROM rfqhub_bundles_maker_1 AS m WHERE m.source = v_maker.id) THEN
		RAISE EXCEPTION 'aggregate for maker % was already consumed as a source', p_bundle_maker_id;
	END IF;

	UPDATE rfqhub_bundles_taker_1 AS t
	SET was_offramped = TRUE,
		updated_by = CURRENT_TIMESTAMP
	WHERE t.id = v_taker.id;

	UPDATE rfqhub_bundles_maker_1 AS m
	SET was_offramped = TRUE,
		updated_by = CURRENT_TIMESTAMP
	WHERE m.id = v_maker.id;

	RETURN QUERY SELECT v_taker.end_source_amt, v_maker.end_source_amt;
END;
$$;

CREATE FUNCTION rfqhub_get_taker_leftover_1 (
	p_bundle_taker_id INTEGER
)
RETURNS BIGINT
LANGUAGE sql
STABLE
AS $$
	SELECT t.end_source_amt
	FROM rfqhub_bundles_taker_1 AS t
	WHERE t.id = p_bundle_taker_id;
$$;

-- migrate:down

DROP FUNCTION IF EXISTS rfqhub_get_taker_leftover_1;
DROP FUNCTION IF EXISTS rfqhub_mark_as_offramped_1;
DROP FUNCTION IF EXISTS rfqhub_resolve_aggregate_1;
DROP FUNCTION IF EXISTS rfqhub_pick_maker_1;
DROP FUNCTION IF EXISTS rfqhub_create_bundle_maker_from_aggregate_1;
DROP FUNCTION IF EXISTS rfqhub_set_bundle_maker_user_sig_1;
DROP FUNCTION IF EXISTS rfqhub_create_bundle_maker_onramping_1;
DROP FUNCTION IF EXISTS rfqhub_create_bundle_taker_from_aggregate_1;
DROP FUNCTION IF EXISTS rfqhub_set_bundle_taker_user_sig_1;
DROP FUNCTION IF EXISTS rfqhub_create_bundle_taker_onramping_1;
DROP FUNCTION IF EXISTS rfqhub_get_or_create_timestamplet_1;
DROP FUNCTION IF EXISTS rfqhub_get_spendable_1;
DROP FUNCTION IF EXISTS rfqhub_inspect_bundle_taker_1;

DROP VIEW IF EXISTS rfqhub_bundles_taker_ongoing_1;

ALTER TABLE IF EXISTS rfqhub_bundles_taker_1
	DROP CONSTRAINT IF EXISTS rfqhub_bundles_taker_maker_fk;

DROP TABLE IF EXISTS rfqhub_bundles_maker_1;
DROP TABLE IF EXISTS rfqhub_bundles_maker_user_count_1;
DROP TABLE IF EXISTS rfqhub_bundles_taker_1;
DROP TABLE IF EXISTS rfqhub_timestamplets_1;
DROP TABLE IF EXISTS rfqhub_bundles_taker_user_count_1;

DROP FUNCTION IF EXISTS rfqhub_protect_immutable_timestamplet_1;
DROP TYPE IF EXISTS RFQHUB_OUTCOME;

-- The BalanceChanged table is external, so remove only the index created by
-- this migration and leave the table itself untouched.
DROP INDEX IF EXISTS rfqhub_events_balance_changed_latest_idx;
