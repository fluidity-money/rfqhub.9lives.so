-- migrate:up

-- Bundle creation is deliberately performed in database functions. The functions
-- serialize balance allocation for an account and asset, calculate the amount that
-- can still be reserved, and insert the bundle in one transaction.

CREATE TABLE rfqhub_bundles_taker_user_count_1 (
	id SERIAL PRIMARY KEY,
	addr ADDRESS NOT NULL UNIQUE,
	count INTEGER NOT NULL DEFAULT 0
);

CREATE TYPE RFQHUB_OUTCOME AS ENUM ('Bitcoin');

-- A BundleTaker can take its liquidity directly from the account's onchain pot
-- (source is NULL), or from the taker result of an earlier aggregate (source is
-- the earlier taker row). The latter mirrors OnrampingSource::AggregatedResult.
CREATE TABLE rfqhub_bundles_taker_1 (
	id SERIAL PRIMARY KEY,
	created_by TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
	updated_by TIMESTAMP,
	-- The account owner of this record.
	acc_addr ADDRESS NOT NULL,
	-- Count of the bundle per user derived from rfqhub_bundles_taker_user_count_1.
	user_id INTEGER NOT NULL,
	-- NULL for OnchainPot; the prior taker bundle for AggregatedResult.
	source INTEGER,
	asset ADDRESS NOT NULL,
	-- Compensation paid to the maker if the taker is incorrect.
	amt BIGINT NOT NULL CHECK (amt >= 0),
	is_up BOOLEAN NOT NULL,
	price_target BIGINT NOT NULL CHECK (price_target >= 0),
	outcome RFQHUB_OUTCOME NOT NULL,
	-- The maker selected for the aggregate.
	maker_id INTEGER,
	-- The 48-bit millisecond nonce is represented as a signed SQL BIGINT.
	ms_ts BIGINT NOT NULL,
	expiry TIMESTAMP NOT NULL,
	-- BundleTaker.sig in the Rust representation.
	user_sig BYTES64 NOT NULL UNIQUE,
	-- Set when this aggregate has been made available for offramping.
	was_offramped BOOLEAN NOT NULL DEFAULT FALSE,
	-- Amount allocated to the taker after resolving the aggregate.
	end_source_amt BIGINT CHECK (end_source_amt >= 0),
	UNIQUE (acc_addr, user_id)
);

-- An aggregate result is all-or-nothing in the Rust implementation. A previous
-- taker result therefore cannot be used as the source of two later bundles.
CREATE UNIQUE INDEX rfqhub_bundles_taker_source_uq
	ON rfqhub_bundles_taker_1 (source)
	WHERE source IS NOT NULL;

CREATE INDEX rfqhub_bundles_taker_open_balance_idx
	ON rfqhub_bundles_taker_1 (acc_addr, asset)
	WHERE source IS NULL AND was_offramped = FALSE;

CREATE TABLE rfqhub_bundles_maker_user_count_1 (
	id SERIAL PRIMARY KEY,
	addr ADDRESS NOT NULL UNIQUE,
	count INTEGER NOT NULL DEFAULT 0
);

-- BundleMaker has no OnrampingSource in the Rust type. Its amount always comes
-- from the maker's onramped balance.
CREATE TABLE rfqhub_bundles_maker_1 (
	id SERIAL PRIMARY KEY,
	created_by TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
	updated_by TIMESTAMP,
	acc_addr ADDRESS NOT NULL,
	user_id INTEGER NOT NULL,
	bundle_taker_id INTEGER NOT NULL,
	ms_ts BIGINT NOT NULL,
	-- Compensation paid to the taker if the taker is correct.
	amt BIGINT NOT NULL CHECK (amt >= 0),
	-- Whether the maker was correct after the aggregate was resolved.
	was_correct BOOLEAN,
	-- Whether this maker was selected as the other side of the taker bundle.
	was_chosen BOOLEAN,
	-- Identifies the timestamplet whose fields are embedded in AggregateArgs.
	timestamplet_id INTEGER,
	-- BundleMaker.sig in the Rust representation.
	user_sig BYTES64 NOT NULL UNIQUE,
	-- Aggregate.server_sig. It is stored on the selected maker because an
	-- Aggregate contains exactly one BundleMaker, which embeds its BundleTaker.
	server_sig BYTES64,
	-- Set when this aggregate has been made available for offramping.
	was_offramped BOOLEAN NOT NULL DEFAULT FALSE,
	-- Amount allocated to the maker after resolving the aggregate.
	end_source_amt BIGINT CHECK (end_source_amt >= 0),
	UNIQUE (acc_addr, user_id)
);

-- migrate:down
