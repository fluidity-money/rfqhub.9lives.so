-- Copyright 2026 Fluid Assets Limited. All rights reserved. Use of this
-- source code is governed by a BUSL license that can be found in the
-- LICENSE file.

-- migrate:up

-- BEGIN TEMPORARY: convert old absolute events to deltas until the contract upgrade.
-- Start with an empty event table; ingest deduplicated events in chain order.
-- Decreases produce negative deltas. Remove this block for additive events.
ALTER TABLE rfqhub_events_balance_changed ADD COLUMN temporary_absolute_amount NUMERIC;
CREATE FUNCTION temporary_balance_changed_to_delta_1()
RETURNS TRIGGER LANGUAGE plpgsql AS $$
DECLARE
	v_previous NUMERIC;
BEGIN
	PERFORM pg_advisory_xact_lock(hashtextextended(
		NEW.account_addr::TEXT || ':' || NEW.asset::TEXT, 1785995838));
	SELECT e.temporary_absolute_amount INTO v_previous
	FROM rfqhub_events_balance_changed AS e
	WHERE e.account_addr = NEW.account_addr AND e.asset = NEW.asset
	ORDER BY e.id DESC LIMIT 1;
	NEW.temporary_absolute_amount := NEW.amount;
	NEW.amount := NEW.amount - COALESCE(v_previous, 0);
	RETURN NEW;
END;
$$;
CREATE TRIGGER temporary_balance_changed_to_delta_1
BEFORE INSERT ON rfqhub_events_balance_changed
FOR EACH ROW EXECUTE FUNCTION temporary_balance_changed_to_delta_1();
-- END TEMPORARY

-- BalanceChanged amounts are additive credits after the temporary conversion.
CREATE INDEX rfqhub_events_balance_changed_account_asset_idx
	ON rfqhub_events_balance_changed (account_addr, asset)
	INCLUDE (amount);

-- SUM(BIGINT) stays NUMERIC here; BIGINT consumers reject out-of-range totals.
CREATE VIEW rfqhub_onramped_balances_1 AS
SELECT account_addr, asset, SUM(amount) AS amount
FROM rfqhub_events_balance_changed
GROUP BY account_addr, asset;

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

-- source is retained for the legacy single-aggregate constructor. New bundles
-- use rfqhub_bundles_taker_sources_1 for any number of previous winnings.
-- pot_funded_amt alone reserves onchain funds; aggregate remainders are never
-- credited locally (only a later BalanceChanged event can credit the pot).
CREATE TABLE rfqhub_bundles_taker_1 (
	id SERIAL PRIMARY KEY,
	created_by TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
	updated_by TIMESTAMP,
	acc_addr ADDRESS NOT NULL,
	user_id INTEGER NOT NULL,
	source INTEGER REFERENCES rfqhub_bundles_taker_1 (id),
	-- Historical source links survive release; released auctions cannot accept makers.
	sources_released BOOLEAN NOT NULL DEFAULT FALSE,
	asset ADDRESS NOT NULL,
	amt BIGINT NOT NULL CHECK (amt >= 0),
	pot_funded_amt BIGINT NOT NULL DEFAULT 0
		CHECK (pot_funded_amt >= 0 AND pot_funded_amt <= amt),
	CHECK (source IS NULL OR pot_funded_amt = 0),
	min_offer BIGINT CHECK (min_offer >= 0),
	-- Fraction removed from the remaining minimum per whole second.
	decay_rate NUMERIC NOT NULL DEFAULT 0 CHECK (decay_rate >= 0 AND decay_rate <= 1),
	is_up BOOLEAN NOT NULL,
	price_target BIGINT NOT NULL CHECK (price_target >= 0),
	outcome RFQHUB_OUTCOME NOT NULL,
	maker_id INTEGER,
	ms_ts BIGINT NOT NULL,
	deadline TIMESTAMP NOT NULL,
	expiry TIMESTAMP NOT NULL,
	structure JSON,
	was_offramped BOOLEAN NOT NULL DEFAULT FALSE,
	end_source_amt BIGINT CHECK (end_source_amt >= 0),
	CHECK (deadline <= expiry),
	UNIQUE (acc_addr, user_id)
);

-- An aggregate result is all-or-nothing, with at most one active allocation.
-- Retain legacy source links after an expired, makerless auction releases them.
CREATE UNIQUE INDEX rfqhub_bundles_taker_source_uq
	ON rfqhub_bundles_taker_1 (source)
	WHERE source IS NOT NULL AND NOT sources_released;

-- Historical name: this shared winnings ledger supports either prior winning
-- side and either new role. source_taker_id is the aggregate parent identity,
-- not the winner's role; one winner per aggregate means one active source key.
-- The legacy source pointer is not an additional reservation. Released rows
-- retain amounts and remainders for reconstruction; uniqueness spans both roles.
CREATE TABLE rfqhub_bundles_taker_sources_1 (
	id BIGSERIAL PRIMARY KEY,
	bundle_taker_id INTEGER REFERENCES rfqhub_bundles_taker_1 (id),
	-- Exactly one consumer; maker FK is installed after its table is created.
	bundle_maker_id INTEGER,
	source_taker_id INTEGER NOT NULL REFERENCES rfqhub_bundles_taker_1 (id),
	source_amt BIGINT NOT NULL CHECK (source_amt > 0),
	amt_to_onramp_amt BIGINT NOT NULL CHECK (amt_to_onramp_amt >= 0),
	released_at TIMESTAMP,
	UNIQUE (bundle_taker_id, source_taker_id),
	UNIQUE (bundle_maker_id, source_taker_id),
	CHECK ((bundle_taker_id IS NULL) <> (bundle_maker_id IS NULL)),
	CHECK (bundle_taker_id <> source_taker_id)
);
CREATE UNIQUE INDEX rfqhub_bundles_taker_sources_active_uq
	ON rfqhub_bundles_taker_sources_1 (source_taker_id)
	WHERE released_at IS NULL;
CREATE INDEX rfqhub_bundles_taker_sources_bundle_idx
	ON rfqhub_bundles_taker_sources_1 (bundle_taker_id);

CREATE INDEX rfqhub_bundles_taker_open_balance_idx
	ON rfqhub_bundles_taker_1 (acc_addr, asset)
	INCLUDE (id, pot_funded_amt, deadline)
	WHERE pot_funded_amt > 0;

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
	pot_funded_amt BIGINT NOT NULL DEFAULT 0
		CHECK (pot_funded_amt >= 0 AND pot_funded_amt <= amt),
	CHECK (source IS NULL OR pot_funded_amt = 0),
	was_correct BOOLEAN,
	was_chosen BOOLEAN,
	timestamplet_id INTEGER REFERENCES rfqhub_timestamplets_1 (id),
	structure JSON,
	was_offramped BOOLEAN NOT NULL DEFAULT FALSE,
	end_source_amt BIGINT CHECK (end_source_amt >= 0),
	UNIQUE (acc_addr, user_id)
);

ALTER TABLE rfqhub_bundles_taker_sources_1
	ADD FOREIGN KEY (bundle_maker_id) REFERENCES rfqhub_bundles_maker_1 (id);
CREATE INDEX rfqhub_bundles_taker_sources_maker_idx
	ON rfqhub_bundles_taker_sources_1 (bundle_maker_id);

CREATE UNIQUE INDEX rfqhub_bundles_maker_source_uq
	ON rfqhub_bundles_maker_1 (source)
	WHERE source IS NOT NULL;

CREATE INDEX rfqhub_bundles_maker_taker_idx
	ON rfqhub_bundles_maker_1 (bundle_taker_id);

CREATE INDEX rfqhub_bundles_maker_open_balance_idx
	ON rfqhub_bundles_maker_1 (acc_addr, bundle_taker_id)
	INCLUDE (pot_funded_amt)
	WHERE pot_funded_amt > 0
		AND was_chosen IS DISTINCT FROM FALSE;

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
WHERE t.structure IS NOT NULL
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

-- Return the unreserved sum of BalanceChanged onramp credits. Aggregate
-- results are not included here until a result is partially returned to the pot
-- and recorded by a later BalanceChanged event.
-- Concluding an aggregate or requesting withdrawal is not indexed settlement:
-- both sides' matched pot stakes stay reserved, including losing stakes.
-- BalanceChanged identifies account/asset/onramped amount, not a bundle. Even a
-- newer event for the same account/asset cannot safely release a reservation.
-- Fail closed until bundle-linked settlement evidence is available; do not infer
-- release from event order/timestamps or the local end_source_amt/was_offramped.
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
		FROM rfqhub_onramped_balances_1 AS e
		WHERE e.account_addr = p_acc_addr
			AND e.asset = p_asset
	), 0)
	INTO v_balance;

	SELECT COALESCE(SUM(t.pot_funded_amt), 0)
	INTO v_taker_reserved
	FROM rfqhub_bundles_taker_1 AS t
	WHERE t.acc_addr = p_acc_addr
		AND t.asset = p_asset
		AND t.pot_funded_amt > 0
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

	SELECT COALESCE(SUM(m.pot_funded_amt), 0)
	INTO v_maker_reserved
	FROM rfqhub_bundles_maker_1 AS m
	JOIN rfqhub_bundles_taker_1 AS t ON t.id = m.bundle_taker_id
	WHERE m.acc_addr = p_acc_addr
		AND t.asset = p_asset
		AND m.pot_funded_amt > 0
		AND m.was_chosen IS DISTINCT FROM FALSE;

	RETURN GREATEST(v_balance - v_taker_reserved - v_maker_reserved, 0);
END;
$$;

-- No maker offer (including unsigned/unselected offers) may exist before an
-- expired auction can return its aggregate funding. Do not infer this from
-- maker_id alone, or from conclusion/offramp of a matched result.
CREATE FUNCTION rfqhub_taker_sources_releasable_1(p_bundle_taker_id INTEGER)
RETURNS BOOLEAN LANGUAGE sql STABLE AS $$
	SELECT EXISTS (
		SELECT 1 FROM rfqhub_bundles_taker_1 AS t
		WHERE t.id = p_bundle_taker_id AND t.deadline <= CURRENT_TIMESTAMP
			AND t.maker_id IS NULL AND NOT t.was_offramped
			AND NOT EXISTS (SELECT 1 FROM rfqhub_bundles_maker_1 AS m
				WHERE m.bundle_taker_id = t.id)
	);
$$;

-- Used by snapshot reads and by writers AFTER taking the source parent lock.
-- Expired allocations are already available to reads before lazy release writes.
CREATE FUNCTION rfqhub_taker_source_reserved_1(p_source_taker_id INTEGER)
RETURNS BOOLEAN LANGUAGE sql STABLE AS $$
	SELECT EXISTS (
		SELECT 1 FROM rfqhub_bundles_taker_sources_1 AS s
		WHERE s.source_taker_id = p_source_taker_id AND s.released_at IS NULL
			AND (s.bundle_maker_id IS NOT NULL
				OR NOT rfqhub_taker_sources_releasable_1(s.bundle_taker_id))
	) OR EXISTS (
		SELECT 1 FROM rfqhub_bundles_taker_1 AS child
		WHERE child.source = p_source_taker_id AND NOT child.sources_released
			AND NOT rfqhub_taker_sources_releasable_1(child.id)
	) OR EXISTS (
		-- Legacy maker.source names the prior selected maker, not its parent.
		SELECT 1 FROM rfqhub_bundles_maker_1 AS child
		JOIN rfqhub_bundles_taker_1 AS parent ON parent.maker_id = child.source
		WHERE parent.id = p_source_taker_id
			AND child.was_chosen IS DISTINCT FROM FALSE
	);
$$;

-- Callers lock source parent -> selected maker first, as withdrawal does.
-- Lock each consuming parent against maker constructors' FOR SHARE, then use a
-- separate statement to recheck offers committed while waiting (READ COMMITTED).
-- Mark the auction closed to makers too: a transaction started before its
-- deadline must not insert a late offer after these reservations were released.
CREATE FUNCTION rfqhub_release_taker_source_1(p_source_taker_id INTEGER)
RETURNS VOID LANGUAGE plpgsql AS $$
DECLARE
	v_child_id INTEGER;
BEGIN
	PERFORM 1 FROM rfqhub_bundles_taker_1
	WHERE id = p_source_taker_id FOR UPDATE;
	IF NOT FOUND THEN RETURN; END IF;
	FOR v_child_id IN
		SELECT s.bundle_taker_id FROM rfqhub_bundles_taker_sources_1 AS s
		WHERE s.source_taker_id = p_source_taker_id AND s.released_at IS NULL
			AND s.bundle_taker_id IS NOT NULL
		UNION
		SELECT child.id FROM rfqhub_bundles_taker_1 AS child
		WHERE child.source = p_source_taker_id AND NOT child.sources_released
		ORDER BY 1
	LOOP
		PERFORM 1 FROM rfqhub_bundles_taker_1 WHERE id = v_child_id FOR UPDATE;
		IF rfqhub_taker_sources_releasable_1(v_child_id) THEN
			UPDATE rfqhub_bundles_taker_1 SET sources_released = TRUE
			WHERE id = v_child_id;
			UPDATE rfqhub_bundles_taker_sources_1 SET released_at = CURRENT_TIMESTAMP
			WHERE bundle_taker_id = v_child_id AND source_taker_id = p_source_taker_id
				AND released_at IS NULL;
		END IF;
	END LOOP;
END;
$$;

-- Read-only snapshot of available funding, not a reservation or guarantee.
-- Keep source eligibility identical to rfqhub_create_bundle_taker_1's candidate
-- query. Either prior winning side is eligible; get_spendable stays pot-only
-- so constructors cannot count these aggregate sources twice.
CREATE FUNCTION rfqhub_get_available_balance_1 (
	p_acc_addr ADDRESS,
	p_asset ADDRESS
)
RETURNS BIGINT
LANGUAGE sql
STABLE
AS $$
	SELECT (rfqhub_get_spendable_1(p_acc_addr, p_asset)
		+ COALESCE(SUM(CASE WHEN m.was_correct THEN m.end_source_amt ELSE t.end_source_amt END), 0))::BIGINT
	FROM rfqhub_bundles_taker_1 AS t
	JOIN rfqhub_bundles_maker_1 AS m
		ON m.id = t.maker_id AND m.bundle_taker_id = t.id
	JOIN rfqhub_timestamplets_1 AS ts ON ts.id = m.timestamplet_id
	WHERE CASE WHEN m.was_correct THEN m.acc_addr ELSE t.acc_addr END = p_acc_addr AND t.asset = p_asset
		AND CASE WHEN m.was_correct THEN m.end_source_amt ELSE t.end_source_amt END > 0
		AND NOT CASE WHEN m.was_correct THEN m.was_offramped ELSE t.was_offramped END
		AND t.structure IS NOT NULL AND m.structure IS NOT NULL
		AND m.was_chosen = TRUE AND m.was_correct IS NOT NULL
		AND NOT rfqhub_taker_source_reserved_1(t.id);
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

-- One row per funding source, ordered by prior result size/id, pot last.
-- amt is the total bundle funding, repeated on every output row.
CREATE FUNCTION rfqhub_create_bundle_taker_1 (
	p_acc_addr ADDRESS,
	p_asset ADDRESS,
	p_min_amt BIGINT,
	p_max_amt BIGINT,
	p_is_up BOOLEAN,
	p_price_target BIGINT,
	p_outcome RFQHUB_OUTCOME,
	p_ms_ts BIGINT,
	p_expiry INTEGER,
	p_min_offer BIGINT,
	p_deadline INTEGER,
	p_decay_rate NUMERIC DEFAULT 0
)
RETURNS TABLE (
	bundle_taker_id INTEGER,
	amt BIGINT,
	source_amt BIGINT,
	source_maker_id INTEGER,
	bundle_maker_structure JSON,
	timestamplet_id INTEGER,
	timestamplet_end_value BIGINT,
	timestamplet_time TIMESTAMP,
	timestamplet_outcome RFQHUB_OUTCOME
)
LANGUAGE plpgsql
AS $$
DECLARE
	v_available BIGINT;
	v_amt BIGINT := 0;
	v_pot_amt BIGINT;
	v_take BIGINT;
	v_winner_amt BIGINT;
	v_winner_acc ADDRESS;
	v_winner_offramped BOOLEAN;
	v_user_id INTEGER;
	v_bundle_taker_id INTEGER;
	v_expiry TIMESTAMP;
	v_deadline TIMESTAMP;
	v_candidate RECORD;
	v_source rfqhub_bundles_taker_1%ROWTYPE;
	v_maker rfqhub_bundles_maker_1%ROWTYPE;
	v_source_ids INTEGER[] := ARRAY[]::INTEGER[];
	v_source_amts BIGINT[] := ARRAY[]::BIGINT[];
	v_remainders BIGINT[] := ARRAY[]::BIGINT[];
BEGIN
	IF p_min_amt IS NULL OR p_max_amt IS NULL OR p_min_amt <= 0 OR p_max_amt < p_min_amt THEN
		RAISE EXCEPTION 'invalid taker amount range: minimum %, maximum %', p_min_amt, p_max_amt;
	END IF;
	IF p_decay_rate IS NULL OR NOT (p_decay_rate >= 0 AND p_decay_rate <= 1) THEN
		RAISE EXCEPTION 'decay rate must be between zero and one: %', p_decay_rate;
	END IF;
	IF p_min_offer < 0 THEN
		RAISE EXCEPTION 'minimum offer cannot be negative: %', p_min_offer;
	END IF;
	v_expiry := TO_TIMESTAMP(p_expiry) AT TIME ZONE 'UTC';
	IF p_deadline IS NULL THEN
		RAISE EXCEPTION 'taker deadline cannot be null';
	END IF;
	v_deadline := TO_TIMESTAMP(p_deadline) AT TIME ZONE 'UTC';
	IF v_deadline > v_expiry THEN
		RAISE EXCEPTION 'taker deadline % cannot be after expiry %', v_deadline, v_expiry;
	END IF;

	-- All pot constructors use the same account-scoped transaction lock, BEFORE
	-- any row locks or balance reads. Hash collisions only serialize accounts.
	PERFORM pg_advisory_xact_lock(hashtextextended(p_acc_addr::TEXT, 1785995837));

	FOR v_candidate IN
		SELECT t.id
		FROM rfqhub_bundles_taker_1 AS t
		JOIN rfqhub_bundles_maker_1 AS m
			ON m.id = t.maker_id AND m.bundle_taker_id = t.id
		JOIN rfqhub_timestamplets_1 AS ts ON ts.id = m.timestamplet_id
		WHERE CASE WHEN m.was_correct THEN m.acc_addr ELSE t.acc_addr END = p_acc_addr AND t.asset = p_asset
			AND CASE WHEN m.was_correct THEN m.end_source_amt ELSE t.end_source_amt END > 0
			AND NOT CASE WHEN m.was_correct THEN m.was_offramped ELSE t.was_offramped END
			AND t.structure IS NOT NULL AND m.structure IS NOT NULL
			AND m.was_chosen = TRUE AND m.was_correct IS NOT NULL
			AND NOT rfqhub_taker_source_reserved_1(t.id)
		ORDER BY CASE WHEN m.was_correct THEN m.end_source_amt ELSE t.end_source_amt END ASC, t.id ASC
	LOOP
		EXIT WHEN v_amt = p_max_amt;
		-- Never lock a joined maker before its parent. Offramp and both taker
		-- reinvestment paths serialize on this parent row. Refresh all eligibility
		-- after waiting, including consumption inserts made by a competing call.
		SELECT t.* INTO v_source FROM rfqhub_bundles_taker_1 AS t
		WHERE t.id = v_candidate.id FOR UPDATE;
		IF NOT FOUND THEN
			CONTINUE;
		END IF;
		SELECT m.* INTO v_maker FROM rfqhub_bundles_maker_1 AS m
		WHERE m.id = v_source.maker_id FOR UPDATE;
		IF NOT FOUND THEN
			CONTINUE;
		END IF;
		v_winner_amt := CASE WHEN v_maker.was_correct THEN v_maker.end_source_amt ELSE v_source.end_source_amt END;
		v_winner_acc := CASE WHEN v_maker.was_correct THEN v_maker.acc_addr ELSE v_source.acc_addr END;
		v_winner_offramped := CASE WHEN v_maker.was_correct THEN v_maker.was_offramped ELSE v_source.was_offramped END;
		IF v_winner_acc IS DISTINCT FROM p_acc_addr
			OR v_source.asset IS DISTINCT FROM p_asset
			OR v_winner_amt IS NULL OR v_winner_amt <= 0
			OR v_winner_offramped OR v_source.structure IS NULL
			OR v_maker.bundle_taker_id IS DISTINCT FROM v_source.id
			OR v_maker.was_chosen IS DISTINCT FROM TRUE
			OR v_maker.was_correct IS NULL
			OR v_maker.structure IS NULL
			OR NOT EXISTS (SELECT 1 FROM rfqhub_timestamplets_1 AS ts
				WHERE ts.id = v_maker.timestamplet_id)
			OR rfqhub_taker_source_reserved_1(v_source.id) THEN
			CONTINUE;
		END IF;
		PERFORM rfqhub_release_taker_source_1(v_source.id);
		IF rfqhub_taker_source_reserved_1(v_source.id) THEN
			CONTINUE;
		END IF;
		v_take := LEAST(v_winner_amt, p_max_amt - v_amt);
		v_source_ids := array_append(v_source_ids, v_source.id);
		v_source_amts := array_append(v_source_amts, v_take);
		v_remainders := array_append(v_remainders, v_winner_amt - v_take);
		v_amt := v_amt + v_take;
	END LOOP;

	v_available := rfqhub_get_spendable_1(p_acc_addr, p_asset);
	v_pot_amt := LEAST(p_max_amt - v_amt, v_available);
	v_amt := v_amt + v_pot_amt;
	IF v_amt < p_min_amt THEN
		RAISE EXCEPTION 'insufficient taker funding: available %, minimum %', v_amt, p_min_amt;
	END IF;

	INSERT INTO rfqhub_bundles_taker_user_count_1 AS c (addr, count)
	VALUES (p_acc_addr, 1)
	ON CONFLICT (addr) DO UPDATE SET count = c.count + 1
	RETURNING count INTO v_user_id;

	INSERT INTO rfqhub_bundles_taker_1 (
		acc_addr, user_id, source, asset, amt, pot_funded_amt, min_offer, decay_rate, is_up,
		price_target, outcome, ms_ts, deadline, expiry
	)
	VALUES (
		p_acc_addr, v_user_id, NULL, p_asset, v_amt, v_pot_amt, p_min_offer, p_decay_rate,
		p_is_up, p_price_target, p_outcome, p_ms_ts, v_deadline, v_expiry
	)
	RETURNING id INTO v_bundle_taker_id;

	INSERT INTO rfqhub_bundles_taker_sources_1 (
		bundle_taker_id, source_taker_id, source_amt, amt_to_onramp_amt
	)
	SELECT v_bundle_taker_id, u.source_id, u.taken, u.remainder
	FROM unnest(v_source_ids, v_source_amts, v_remainders) AS u(source_id, taken, remainder);

	RETURN QUERY
	SELECT v_bundle_taker_id, v_amt,
		u.taken, m.id, m.structure,
		ts.id, ts.timestamplet_val, ts.timestamplet_time, ts.timestamplet_outcome
	FROM unnest(v_source_ids, v_source_amts, v_remainders)
		WITH ORDINALITY AS u(source_id, taken, remainder, ord)
	JOIN rfqhub_bundles_taker_1 AS t ON t.id = u.source_id
	JOIN rfqhub_bundles_maker_1 AS m ON m.id = t.maker_id
	JOIN rfqhub_timestamplets_1 AS ts ON ts.id = m.timestamplet_id
	ORDER BY u.ord;

	IF v_pot_amt > 0 THEN
		RETURN QUERY SELECT v_bundle_taker_id, v_amt,
			v_pot_amt, NULL::INTEGER, NULL::JSON,
			NULL::INTEGER, NULL::BIGINT, NULL::TIMESTAMP, NULL::RFQHUB_OUTCOME;
	END IF;
END;
$$;

-- Earlier versions of this function attached a user signature. It now stores the
-- taker's fully signed bundle structure as raw JSON. The database never
-- interprets the JSON; it exists only so the structure can be reconstructed for
-- signature derivation. The column stays nullable and is set once signing completes.
CREATE FUNCTION rfqhub_set_bundle_taker_structure_1 (
	p_bundle_taker_id INTEGER,
	p_structure JSON
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
	IF p_structure IS NULL THEN
		RAISE EXCEPTION 'taker bundle structure cannot be null';
	END IF;

	UPDATE rfqhub_bundles_taker_1
	SET structure = p_structure,
		updated_by = CURRENT_TIMESTAMP
	WHERE id = p_bundle_taker_id;

	IF NOT FOUND THEN
		RAISE EXCEPTION 'bundle taker % does not exist', p_bundle_taker_id;
	END IF;
END;
$$;

-- Either prior winning side can fund either new role. The historical ledger
-- name source_taker_id denotes the aggregate parent, not the winning side:
-- there is only one winner per aggregate, hence one active reservation key.
CREATE FUNCTION rfqhub_create_bundle_maker_1 (
	p_acc_addr ADDRESS,
	p_bundle_taker_id INTEGER,
	p_min_amt BIGINT,
	p_max_amt BIGINT,
	p_ms_ts BIGINT
)
RETURNS TABLE (
	bundle_maker_id INTEGER,
	maker_amt BIGINT,
	taker_outcome RFQHUB_OUTCOME,
	structure_bundle_taker JSON,
	source_amt BIGINT,
	source_maker_id INTEGER,
	bundle_maker_structure JSON,
	timestamplet_id INTEGER,
	timestamplet_end_value BIGINT,
	timestamplet_time TIMESTAMP,
	timestamplet_outcome RFQHUB_OUTCOME
)
LANGUAGE plpgsql
AS $$
DECLARE
	v_taker rfqhub_bundles_taker_1%ROWTYPE;
	v_min_offer NUMERIC;
	v_elapsed_seconds NUMERIC;
	v_available BIGINT;
	v_amt BIGINT := 0;
	v_pot_amt BIGINT;
	v_take BIGINT;
	v_winner_amt BIGINT;
	v_winner_acc ADDRESS;
	v_winner_offramped BOOLEAN;
	v_user_id INTEGER;
	v_bundle_maker_id INTEGER;
	v_candidate RECORD;
	v_source rfqhub_bundles_taker_1%ROWTYPE;
	v_maker rfqhub_bundles_maker_1%ROWTYPE;
	v_source_ids INTEGER[] := ARRAY[]::INTEGER[];
	v_source_amts BIGINT[] := ARRAY[]::BIGINT[];
	v_remainders BIGINT[] := ARRAY[]::BIGINT[];
BEGIN
	IF p_min_amt IS NULL OR p_max_amt IS NULL OR p_min_amt <= 0 OR p_max_amt < p_min_amt THEN
		RAISE EXCEPTION 'invalid maker amount range: minimum %, maximum %', p_min_amt, p_max_amt;
	END IF;

	-- Acquire the same lock as taker creation before taking any parent row lock.
	PERFORM pg_advisory_xact_lock(hashtextextended(p_acc_addr::TEXT, 1785995837));
	SELECT t.* INTO STRICT v_taker
	FROM rfqhub_bundles_taker_1 AS t
	WHERE t.id = p_bundle_taker_id
	FOR SHARE;
	IF v_taker.acc_addr = p_acc_addr THEN
		RAISE EXCEPTION 'maker owner cannot match taker owner for taker %', p_bundle_taker_id;
	END IF;
	IF v_taker.maker_id IS NOT NULL THEN
		RAISE EXCEPTION 'taker % already selected maker %', p_bundle_taker_id, v_taker.maker_id;
	END IF;
	IF v_taker.deadline <= CURRENT_TIMESTAMP OR v_taker.sources_released THEN
		RAISE EXCEPTION 'taker % deadline has passed', p_bundle_taker_id;
	END IF;
	IF v_taker.structure IS NULL THEN
		RAISE EXCEPTION 'taker % has no signed structure', p_bundle_taker_id;
	END IF;

	FOR v_candidate IN
		SELECT t.id
		FROM rfqhub_bundles_taker_1 AS t
		JOIN rfqhub_bundles_maker_1 AS m
			ON m.id = t.maker_id AND m.bundle_taker_id = t.id
		JOIN rfqhub_timestamplets_1 AS ts ON ts.id = m.timestamplet_id
		WHERE CASE WHEN m.was_correct THEN m.acc_addr ELSE t.acc_addr END = p_acc_addr AND t.asset = v_taker.asset
			AND CASE WHEN m.was_correct THEN m.end_source_amt ELSE t.end_source_amt END > 0
			AND NOT CASE WHEN m.was_correct THEN m.was_offramped ELSE t.was_offramped END
			AND t.structure IS NOT NULL AND m.structure IS NOT NULL
			AND m.was_chosen = TRUE AND m.was_correct IS NOT NULL
			AND NOT rfqhub_taker_source_reserved_1(t.id)
		ORDER BY CASE WHEN m.was_correct THEN m.end_source_amt ELSE t.end_source_amt END ASC, t.id ASC
	LOOP
		EXIT WHEN v_amt = p_max_amt;
		SELECT t.* INTO v_source FROM rfqhub_bundles_taker_1 AS t
		WHERE t.id = v_candidate.id FOR UPDATE;
		IF NOT FOUND THEN CONTINUE; END IF;
		SELECT m.* INTO v_maker FROM rfqhub_bundles_maker_1 AS m
		WHERE m.id = v_source.maker_id FOR UPDATE;
		IF NOT FOUND THEN CONTINUE; END IF;
		v_winner_amt := CASE WHEN v_maker.was_correct THEN v_maker.end_source_amt ELSE v_source.end_source_amt END;
		v_winner_acc := CASE WHEN v_maker.was_correct THEN v_maker.acc_addr ELSE v_source.acc_addr END;
		v_winner_offramped := CASE WHEN v_maker.was_correct THEN v_maker.was_offramped ELSE v_source.was_offramped END;
		IF v_winner_acc IS DISTINCT FROM p_acc_addr
			OR v_source.asset IS DISTINCT FROM v_taker.asset
			OR v_winner_amt IS NULL OR v_winner_amt <= 0
			OR v_winner_offramped OR v_source.structure IS NULL
			OR v_maker.bundle_taker_id IS DISTINCT FROM v_source.id
			OR v_maker.was_chosen IS DISTINCT FROM TRUE
			OR v_maker.was_correct IS NULL
			OR v_maker.structure IS NULL
			OR NOT EXISTS (SELECT 1 FROM rfqhub_timestamplets_1 AS ts
				WHERE ts.id = v_maker.timestamplet_id)
			OR rfqhub_taker_source_reserved_1(v_source.id) THEN
			CONTINUE;
		END IF;
		PERFORM rfqhub_release_taker_source_1(v_source.id);
		IF rfqhub_taker_source_reserved_1(v_source.id) THEN CONTINUE; END IF;
		v_take := LEAST(v_winner_amt, p_max_amt - v_amt);
		v_source_ids := array_append(v_source_ids, v_source.id);
		v_source_amts := array_append(v_source_amts, v_take);
		v_remainders := array_append(v_remainders, v_winner_amt - v_take);
		v_amt := v_amt + v_take;
	END LOOP;

	v_available := rfqhub_get_spendable_1(p_acc_addr, v_taker.asset);
	v_pot_amt := LEAST(p_max_amt - v_amt, v_available);
	v_amt := v_amt + v_pot_amt;
	IF v_amt < p_min_amt THEN
		RAISE EXCEPTION 'insufficient maker funding: available %, minimum %', v_amt, p_min_amt;
	END IF;
	-- Compute decay on demand; never mutate the original minimum. NULL means zero.
	-- Keep fractional token units for comparison rather than rounding the floor.
	v_elapsed_seconds := GREATEST(0, FLOOR(EXTRACT(EPOCH FROM
		(LEAST(CURRENT_TIMESTAMP AT TIME ZONE 'UTC', v_taker.deadline) - v_taker.created_by))));
	v_min_offer := CASE
		WHEN v_elapsed_seconds = 0 THEN COALESCE(v_taker.min_offer, 0)
		ELSE COALESCE(v_taker.min_offer, 0) * POWER(1 - v_taker.decay_rate, v_elapsed_seconds)
	END;
	IF v_amt <= v_min_offer THEN
		RAISE EXCEPTION 'maker offer % must exceed taker minimum offer %', v_amt, v_min_offer;
	END IF;
	INSERT INTO rfqhub_bundles_maker_user_count_1 AS c (addr, count)
	VALUES (p_acc_addr, 1)
	ON CONFLICT (addr) DO UPDATE SET count = c.count + 1
	RETURNING count INTO v_user_id;

	INSERT INTO rfqhub_bundles_maker_1 (
		acc_addr, user_id, source, bundle_taker_id, ms_ts, amt, pot_funded_amt
	)
	VALUES (
		p_acc_addr, v_user_id, NULL, p_bundle_taker_id, p_ms_ts, v_amt, v_pot_amt
	)
	RETURNING id INTO v_bundle_maker_id;

	INSERT INTO rfqhub_bundles_taker_sources_1 (
		bundle_maker_id, source_taker_id, source_amt, amt_to_onramp_amt
	)
	SELECT v_bundle_maker_id, u.source_id, u.taken, u.remainder
	FROM unnest(v_source_ids, v_source_amts, v_remainders) AS u(source_id, taken, remainder);

	-- maker_amt is repeated bundle total; only source_amt is additive.
	RETURN QUERY
	SELECT v_bundle_maker_id, v_amt, v_taker.outcome, v_taker.structure,
		u.taken, m.id, m.structure,
		ts.id, ts.timestamplet_val, ts.timestamplet_time, ts.timestamplet_outcome
	FROM unnest(v_source_ids, v_source_amts) WITH ORDINALITY AS u(source_id, taken, ord)
	JOIN rfqhub_bundles_taker_1 AS t ON t.id = u.source_id
	JOIN rfqhub_bundles_maker_1 AS m ON m.id = t.maker_id
	JOIN rfqhub_timestamplets_1 AS ts ON ts.id = m.timestamplet_id
	ORDER BY u.ord;
	IF v_pot_amt > 0 THEN
		RETURN QUERY SELECT v_bundle_maker_id, v_amt, v_taker.outcome, v_taker.structure,
			v_pot_amt, NULL::INTEGER, NULL::JSON,
			NULL::INTEGER, NULL::BIGINT, NULL::TIMESTAMP, NULL::RFQHUB_OUTCOME;
	END IF;
END;
$$;

CREATE FUNCTION rfqhub_set_bundle_maker_structure_1 (
	p_bundle_maker_id INTEGER,
	p_structure JSON
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
	IF p_structure IS NULL THEN
		RAISE EXCEPTION 'maker bundle structure cannot be null';
	END IF;

	UPDATE rfqhub_bundles_maker_1
	SET structure = p_structure,
		updated_by = CURRENT_TIMESTAMP
	WHERE id = p_bundle_maker_id;

	IF NOT FOUND THEN
		RAISE EXCEPTION 'bundle maker % does not exist', p_bundle_maker_id;
	END IF;
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
	v_bundle_taker_id INTEGER;
	v_existing_maker_id INTEGER;
BEGIN
	SELECT m.bundle_taker_id INTO STRICT v_bundle_taker_id
	FROM rfqhub_bundles_maker_1 AS m
	WHERE m.id = p_bundle_maker_id;

	SELECT t.maker_id INTO v_existing_maker_id
	FROM rfqhub_bundles_taker_1 AS t
	WHERE t.id = v_bundle_taker_id
	FOR UPDATE;

	SELECT m.* INTO STRICT v_maker
	FROM rfqhub_bundles_maker_1 AS m
	WHERE m.id = p_bundle_maker_id
	FOR UPDATE;

	IF v_maker.bundle_taker_id <> v_bundle_taker_id THEN
		RAISE EXCEPTION 'maker % changed taker', p_bundle_maker_id;
	END IF;

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

	-- Only definitive rejection releases a maker's funding, never mere expiry.
	-- Keep original source amounts/remainders for historical reconstruction.
	UPDATE rfqhub_bundles_taker_sources_1 AS s
	SET released_at = CURRENT_TIMESTAMP
	FROM rfqhub_bundles_maker_1 AS loser
	WHERE s.bundle_maker_id = loser.id AND s.released_at IS NULL
		AND loser.bundle_taker_id = v_maker.bundle_taker_id
		AND loser.was_chosen = FALSE;
END;
$$;

-- Resolve the selected pair after expiry. Taker identity and outcome determine
-- the latest Chainlink observation at or before expiry; the resulting
-- timestamplet is created or reused atomically.
CREATE FUNCTION rfqhub_resolve_aggregate_1 (
	p_bundle_maker_id INTEGER
)
RETURNS TABLE (
	bundle_taker_id INTEGER,
	bundle_maker_id INTEGER,
	taker_acc_addr ADDRESS,
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
	v_bundle_taker_id INTEGER;
	v_timestamplet_id INTEGER;
	v_timestamplet_val BIGINT;
	v_oracle_base VARCHAR;
	v_taker_correct BOOLEAN;
	v_total BIGINT;
	v_taker_end BIGINT;
	v_maker_end BIGINT;
	v_winner ADDRESS;
BEGIN
	SELECT m.bundle_taker_id INTO STRICT v_bundle_taker_id
	FROM rfqhub_bundles_maker_1 AS m
	WHERE m.id = p_bundle_maker_id;

	SELECT t.* INTO STRICT v_taker
	FROM rfqhub_bundles_taker_1 AS t
	WHERE t.id = v_bundle_taker_id
	FOR UPDATE;

	SELECT m.* INTO STRICT v_maker
	FROM rfqhub_bundles_maker_1 AS m
	WHERE m.id = p_bundle_maker_id
	FOR UPDATE;

	IF v_maker.bundle_taker_id <> v_taker.id THEN
		RAISE EXCEPTION 'maker % does not belong to taker %', p_bundle_maker_id, v_taker.id;
	END IF;

	IF v_taker.maker_id IS DISTINCT FROM p_bundle_maker_id OR v_maker.was_chosen IS DISTINCT FROM TRUE THEN
		RAISE EXCEPTION 'maker % is not selected for taker %', p_bundle_maker_id, v_taker.id;
	END IF;

	IF v_maker.timestamplet_id IS NOT NULL THEN
		RETURN QUERY SELECT
			v_taker.id,
			v_maker.id,
			v_taker.acc_addr,
			v_maker.timestamplet_id,
			CASE WHEN v_maker.was_correct THEN v_maker.acc_addr ELSE v_taker.acc_addr END,
			NOT v_maker.was_correct,
			v_taker.end_source_amt,
			v_maker.end_source_amt;
		RETURN;
	END IF;

	IF CURRENT_TIMESTAMP < v_taker.expiry THEN
		RAISE EXCEPTION 'taker % does not expire until %', v_taker.id, v_taker.expiry;
	END IF;

	v_oracle_base := CASE v_taker.outcome
		WHEN 'BitcoinPrice' THEN 'BTC'
		ELSE NULL
	END;
	IF v_oracle_base IS NULL THEN
		RAISE EXCEPTION 'no oracle base mapping for outcome %', v_taker.outcome;
	END IF;

	SELECT p.amount INTO v_timestamplet_val
	FROM oracles_ninelives_prices_3 AS p
	WHERE p.base = v_oracle_base
		AND p.quote = 'USD'
		AND p.source = 'chainlink'
		AND p.created_by <= v_taker.expiry
	ORDER BY p.created_by DESC, p.id DESC
	LIMIT 1;

	IF NOT FOUND THEN
		RAISE EXCEPTION
			'no Chainlink price found for base %, quote USD at or before %',
			v_oracle_base,
			v_taker.expiry;
	END IF;

	v_timestamplet_id := rfqhub_get_or_create_timestamplet_1(
		v_taker.expiry,
		v_taker.outcome,
		v_timestamplet_val
	);
	v_taker_correct := (v_timestamplet_val > v_taker.price_target AND v_taker.is_up)
		OR (v_timestamplet_val <= v_taker.price_target AND NOT v_taker.is_up);
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
		end_source_amt = v_maker_end,
		updated_by = CURRENT_TIMESTAMP
	WHERE m.id = v_maker.id;

	RETURN QUERY SELECT
		v_taker.id,
		v_maker.id,
		v_taker.acc_addr,
		v_timestamplet_id,
		v_winner,
		v_taker_correct,
		v_taker_end,
		v_maker_end;
END;
$$;

-- Select the largest signed maker offer for a taker, record the supplied
-- timestamplet at the taker's expiry, and resolve the pair in place: the maker's
-- was_correct and both end_source_amt values are set from the outcome. Returns
-- the payload needed for server signing plus the resolution results. An existing
-- selection and identical timestamplet are reused on retries; a later call to
-- rfqhub_resolve_aggregate_1 is idempotent because the maker already carries a
-- timestamplet_id.
CREATE FUNCTION rfqhub_conclude_aggregate_1 (
	p_bundle_taker_id INTEGER,
	p_timestamplet_val BIGINT
)
RETURNS TABLE (
	bundle_maker_id INTEGER,
	taker_was_correct BOOLEAN,
	taker_amt BIGINT,
	maker_amt BIGINT,
	structure JSON
)
LANGUAGE plpgsql
AS $$
DECLARE
	v_taker rfqhub_bundles_taker_1%ROWTYPE;
	v_maker rfqhub_bundles_maker_1%ROWTYPE;
	v_timestamplet_id INTEGER;
	v_taker_correct BOOLEAN;
	v_total BIGINT;
	v_taker_end BIGINT;
	v_maker_end BIGINT;
BEGIN
	SELECT t.* INTO STRICT v_taker
	FROM rfqhub_bundles_taker_1 AS t
	WHERE t.id = p_bundle_taker_id
	FOR UPDATE;

	IF v_taker.structure IS NULL THEN
		RAISE EXCEPTION 'taker % has no signed structure', p_bundle_taker_id;
	END IF;

	IF v_taker.maker_id IS NULL THEN
		SELECT m.* INTO v_maker
		FROM rfqhub_bundles_maker_1 AS m
		WHERE m.bundle_taker_id = v_taker.id
			AND m.structure IS NOT NULL
			AND m.was_chosen IS DISTINCT FROM FALSE
		ORDER BY m.amt DESC, m.id ASC
		LIMIT 1
		FOR UPDATE;
	ELSE
		SELECT m.* INTO v_maker
		FROM rfqhub_bundles_maker_1 AS m
		WHERE m.id = v_taker.maker_id
			AND m.bundle_taker_id = v_taker.id
			AND m.structure IS NOT NULL
		FOR UPDATE;
	END IF;

	IF NOT FOUND THEN
		RAISE EXCEPTION 'no signed maker offers found for taker %', p_bundle_taker_id;
	END IF;

	IF CURRENT_TIMESTAMP < v_taker.expiry + INTERVAL '5 seconds' THEN
		RAISE EXCEPTION
			'taker % cannot be concluded until %',
			p_bundle_taker_id,
			v_taker.expiry + INTERVAL '5 seconds';
	END IF;

	PERFORM rfqhub_pick_maker_1(v_maker.id);
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

	UPDATE rfqhub_bundles_taker_1 AS t
	SET end_source_amt = v_taker_end,
		updated_by = CURRENT_TIMESTAMP
	WHERE t.id = v_taker.id;

	UPDATE rfqhub_bundles_maker_1 AS m
	SET was_correct = NOT v_taker_correct,
		timestamplet_id = v_timestamplet_id,
		end_source_amt = v_maker_end,
		updated_by = CURRENT_TIMESTAMP
	WHERE m.id = v_maker.id;

	RETURN QUERY SELECT
		v_maker.id,
		v_taker_correct,
		v_taker.amt,
		v_maker.amt,
		v_taker.structure;
END;
$$;

-- Mark exactly one resolved winner as offramped. The caller identifies whether
-- it is claiming the taker or maker side, and its account address must match
-- that side of the selected aggregate. Return the selected maker structure and
-- the immutable timestamplet value and timestamp used to resolve the aggregate.
CREATE FUNCTION rfqhub_mark_winner_as_offramped_1 (
	p_acc_addr ADDRESS,
	p_sender_is_taker BOOLEAN,
	p_bundle_taker_id INTEGER
)
RETURNS TABLE (
	bundle_maker_structure JSON,
	timestamplet_end_value BIGINT,
	timestamplet_time TIMESTAMP,
	timestamplet_outcome RFQHUB_OUTCOME
)
LANGUAGE plpgsql
AS $$
DECLARE
	v_maker rfqhub_bundles_maker_1%ROWTYPE;
	v_taker rfqhub_bundles_taker_1%ROWTYPE;
	v_timestamplet_end_value BIGINT;
	v_timestamplet_time TIMESTAMP;
	v_timestamplet_outcome RFQHUB_OUTCOME;
BEGIN
	IF p_sender_is_taker IS NULL THEN
		RAISE EXCEPTION 'sender side cannot be null';
	END IF;

	-- Serialize winner withdrawal with either funding constructor for this owner.
	PERFORM pg_advisory_xact_lock(hashtextextended(p_acc_addr::TEXT, 1785995837));
	SELECT t.* INTO STRICT v_taker
	FROM rfqhub_bundles_taker_1 AS t
	WHERE t.id = p_bundle_taker_id
	FOR UPDATE;

	IF v_taker.maker_id IS NULL THEN
		RAISE EXCEPTION 'taker % has no selected maker', v_taker.id;
	END IF;

	SELECT m.* INTO STRICT v_maker
	FROM rfqhub_bundles_maker_1 AS m
	WHERE m.id = v_taker.maker_id
	FOR UPDATE;

	IF v_taker.maker_id IS DISTINCT FROM v_maker.id
		OR v_maker.bundle_taker_id IS DISTINCT FROM v_taker.id
		OR v_maker.was_chosen IS DISTINCT FROM TRUE THEN
		RAISE EXCEPTION 'maker % is not selected for taker %', v_maker.id, v_taker.id;
	END IF;
	IF v_maker.was_correct IS NULL
		OR v_maker.timestamplet_id IS NULL
		OR v_taker.end_source_amt IS NULL
		OR v_maker.end_source_amt IS NULL THEN
		RAISE EXCEPTION 'aggregate for taker % has not been resolved', v_taker.id;
	END IF;

	SELECT ts.timestamplet_val, ts.timestamplet_time, ts.timestamplet_outcome
	INTO STRICT
		v_timestamplet_end_value,
		v_timestamplet_time,
		v_timestamplet_outcome
	FROM rfqhub_timestamplets_1 AS ts
	WHERE ts.id = v_maker.timestamplet_id;

	IF p_sender_is_taker THEN
		IF v_taker.acc_addr IS DISTINCT FROM p_acc_addr THEN
			RAISE EXCEPTION 'account address does not match bundle taker %', v_taker.id;
		END IF;
		IF v_maker.was_correct IS DISTINCT FROM FALSE OR v_taker.end_source_amt <= 0 THEN
			RAISE EXCEPTION 'taker % is not the winner', v_taker.id;
		END IF;
		PERFORM rfqhub_release_taker_source_1(v_taker.id);
		IF rfqhub_taker_source_reserved_1(v_taker.id) THEN
			RAISE EXCEPTION 'taker result % was already consumed as a source', v_taker.id;
		END IF;

		UPDATE rfqhub_bundles_taker_1 AS t
		SET was_offramped = TRUE,
			updated_by = CURRENT_TIMESTAMP
		WHERE t.id = v_taker.id;
	ELSE
		IF v_maker.acc_addr IS DISTINCT FROM p_acc_addr THEN
			RAISE EXCEPTION 'account address does not match bundle maker %', v_maker.id;
		END IF;
		IF v_maker.was_correct IS DISTINCT FROM TRUE OR v_maker.end_source_amt <= 0 THEN
			RAISE EXCEPTION 'maker % is not the winner', v_maker.id;
		END IF;
		PERFORM rfqhub_release_taker_source_1(v_taker.id);
		IF rfqhub_taker_source_reserved_1(v_taker.id) THEN
			RAISE EXCEPTION 'maker result % was already consumed as a source', v_maker.id;
		END IF;

		UPDATE rfqhub_bundles_maker_1 AS m
		SET was_offramped = TRUE,
			updated_by = CURRENT_TIMESTAMP
		WHERE m.id = v_maker.id;
	END IF;

	RETURN QUERY SELECT
		v_maker.structure,
		v_timestamplet_end_value,
		v_timestamplet_time,
		v_timestamplet_outcome;
END;
$$;

-- migrate:down

DROP FUNCTION IF EXISTS temporary_balance_changed_to_delta_1() CASCADE;

DROP FUNCTION IF EXISTS rfqhub_mark_as_offramped_1;
DROP FUNCTION IF EXISTS rfqhub_mark_winner_as_offramped_1;
DROP FUNCTION IF EXISTS rfqhub_conclude_aggregate_1;
DROP FUNCTION IF EXISTS rfqhub_resolve_aggregate_1;
DROP FUNCTION IF EXISTS rfqhub_pick_maker_1;
DROP FUNCTION IF EXISTS rfqhub_set_bundle_maker_structure_1;
DROP FUNCTION IF EXISTS rfqhub_create_bundle_maker_1;
DROP FUNCTION IF EXISTS rfqhub_set_bundle_taker_structure_1;
DROP FUNCTION IF EXISTS rfqhub_create_bundle_taker_1;
DROP FUNCTION IF EXISTS rfqhub_get_or_create_timestamplet_1;
DROP FUNCTION IF EXISTS rfqhub_get_available_balance_1(ADDRESS, ADDRESS);
DROP FUNCTION IF EXISTS rfqhub_release_taker_source_1(INTEGER);
DROP FUNCTION IF EXISTS rfqhub_taker_source_reserved_1(INTEGER);
DROP FUNCTION IF EXISTS rfqhub_taker_sources_releasable_1(INTEGER);
DROP FUNCTION IF EXISTS rfqhub_get_spendable_1;
DROP FUNCTION IF EXISTS rfqhub_inspect_bundle_taker_1;

DROP VIEW IF EXISTS rfqhub_bundles_taker_ongoing_1;

ALTER TABLE IF EXISTS rfqhub_bundles_taker_1
	DROP CONSTRAINT IF EXISTS rfqhub_bundles_taker_maker_fk;

DROP TABLE IF EXISTS rfqhub_bundles_taker_sources_1;
DROP TABLE IF EXISTS rfqhub_bundles_maker_1;
DROP TABLE IF EXISTS rfqhub_bundles_maker_user_count_1;
DROP TABLE IF EXISTS rfqhub_bundles_taker_1;
DROP TABLE IF EXISTS rfqhub_timestamplets_1;
DROP TABLE IF EXISTS rfqhub_bundles_taker_user_count_1;

DROP FUNCTION IF EXISTS rfqhub_protect_immutable_timestamplet_1;
DROP TYPE IF EXISTS RFQHUB_OUTCOME;

DROP INDEX IF EXISTS rfqhub_oracles_ninelives_prices_3_lookup_idx;
DROP VIEW IF EXISTS rfqhub_onramped_balances_1;
DROP INDEX IF EXISTS rfqhub_events_balance_changed_account_asset_idx;
