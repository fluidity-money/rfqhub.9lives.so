-- migrate:up

-- This function subtracts the latest BalanceChanged(address,address,uint256,uint256)
-- event with any outstanding "Bundles" that were made. These bundles
-- will take the min of this function themselves when they're being
-- created and the amount the user asked when they're being created.

-- The event emits the address of the SENDER, but this address is not
-- the same as the owner of the account. The owner of the Account needs
-- to be cross-referenced to find, so the graph needs to first get the address
-- of the EOA before making any database lookups.

-- These outstanding Onramped coins, which function like coins in the
-- UTXO sense, can be converted to Bundles by creating another row in the
-- database, which is used to then match them with market makers that
-- follow a similar user experience.

-- So, ultimately the two can sort of pair each other out in the
-- recursive reconstruction of the original balance, where an edge worker
-- simulates the bundle's success if it were to be sent on-chain, and
-- then the bundles are used, and a new bundle is made with the two
-- combined together, and the concenated combination of the two is
-- created based on the inputs from the both, so that there's no race
-- issue here.

-- This function gets the outstanding balances for the bundles marked
-- as open:

CREATE TABLE rfqhub_bundles_taker_user_count_1 (
	id SERIAL PRIMARY KEY,
	addr ADDRESS NOT NULL UNIQUE,
	count INTEGER NOT NULL DEFAULT 0
);

CREATE TYPE RFQHUB_OUTCOME AS ENUM ('Bitcoin');

-- BundleTaker bundles are made after validation on the server side. This
-- table is a departure from our usual practice, in that it's updated
-- with whether the bundle is the latest status of it being used in the
-- recursive bundle, aka whether it's the definitive source in the taker.
-- For us to take this, we need to verify the OnchainPotArgs in the mix.
CREATE TABLE rfqhub_bundles_taker_1 (
	id SERIAL PRIMARY KEY,
	created_by TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
	updated_by TIMESTAMP,
	-- The account owner of this record.
	acc_addr ADDRESS NOT NULL,
	-- Count of the bundle per user derived from rfqhub_bundles_taker_user_count_1.
	user_id INTEGER NOT NULL UNIQUE,
	-- Did this BundleTaker come from an onboarding source, or did it come
	-- from an aggregate bundle? NULL if it's the onboarding source.
	source INTEGER,
	-- The asset is derived from the answer that we see after the aggregate is either
	-- submitted, or used in an aggregate bundle.
	asset ADDRESS NOT NULL,
	-- What amount are we able to use with this BundleTaker here? Based on the end
	-- result of the aggregate, or the deposit. This is the compensation we'll be paying a
	-- maker if we're incorrect.
	amt BIGINT NOT NULL,
	-- Is the target of this prediction up or down?
	is_up BOOLEAN NOT NULL,
	-- What's the target here with this all?
	price_target BIGINT NOT NULL,
	-- What's the outcome we're speculating on here?
	outcome RFQHUB_OUTCOME NOT NULL,
	-- The Maker bundle this was combined with, updated on set to make it easier to
	-- interact with the database.
	maker_id INTEGER,
	-- The millisecond timestamp that we use here to prevent abuse.
	ms_ts INTEGER NOT NULL,
	-- When does this option expire and the choice needs to be given according to the user?
	expiry TIMESTAMP NOT NULL,
	-- The user provided signature for this blob:
	user_sig BYTES64 NOT NULL UNIQUE,
	-- Was this offramped?
	was_offramped BOOLEAN NOT NULL DEFAULT FALSE,
	--- The end source amount after the allocation happened at the end of the game.
	end_source_amt INTEGER
);

CREATE TABLE rfqhub_bundles_maker_user_count_1 (
	id SERIAL PRIMARY KEY,
	addr ADDRESS NOT NULL UNIQUE,
	count INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE rfqhub_bundles_maker_1 (
	id SERIAL PRIMARY KEY,
	created_by TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
	updated_by TIMESTAMP,
	-- The account of the maker.
	acc_addr ADDRESS NOT NULL
	-- Count of the bundle per user derived from rfqhub_bundles_maker_user_count_1.
	user_id INTEGER NOT NULL UNIQUE,
	-- The source of where the finances come from that we have here. If NULL, then it
	-- must be an onramping amount. If it's something, this must be the aggregated
	-- bundle.
	source INTEGER,
	-- The BundleTaker this was intended for.
	bundle_taker_id INTEGER NOT NULL,
	-- The millisecond timestamp that we use here.
	ms_ts INTEGER NOT NULL,
	-- The compensation if the taker is correct.
	amt BIGINT NOT NULL
	-- To keep things simple since this is complex code, we actually update this table
	-- with this field. If the choice hasn't happened yet, then we don't proceed yet.
	was_correct BOOLEAN,
	-- Was this bundle chosen in the end to be the other side to a BundleTaker?
	was_chosen BOOLEAN,
	-- Certain fields we get from the underlying taker once, but in the case of this
	-- table, we get the timestamplet value by a left join so we can make use of it here
	timestamplet_id INTEGER,
	-- The server side aggregation signature if this maker was chosen as the winner,
	-- for the aggregate bundle we create. We get the timestamplet fields by looking
	-- up the taker.
	server_sig BYTES64,
	-- Was this offramped?
	was_offramped BOOLEAN NOT NULL DEFAULT FALSE,
	--- The end source amount after the allocation happened at the end of the game.
	end_source_amt INTEGER
);

-- As an example, if we were to create a row for someone that's just
-- opened a position, following from the Alex and Erik example:

-- rfqhub_bundles_taker_1(
-- 	1,				id
-- 	current_timestamp,	created_timestamp
-- 	NULL,			updated timestamp
-- 	<acc addr>,		acc addr
-- 	1,				the count for the user for pagination
-- 	NULL,			was this derived from a bundle? NULL if onramping
-- 	<usdc>,			the asset in use here
-- 	100,				the amount we're using
-- 	TRUE,			are we betting on up or down?
-- 	101,				the price we have to beat
-- 	'Bitcoin',			the outcome stream, aka what price market
-- 	NULL,			the maker id, aka whether this has been used for a maker yet.
-- 	<ms ts>,			the ms timestamp the user provided when signing
-- 	<expiry>,			the expiry of this outcome
-- 	<user sig>			the account's signature
-- );

-- Which would flow into the BundleMaker, assuming they onramped
-- directly:

-- rfqhub_bundles_maker_1(
-- 	1,				id
-- 	current_timestamp,	created timestamp
-- 	NULL,			updated timestamp
-- 	<acc addr>,		acc addr
-- 	1,				user id
-- 	NULL,			where did the liquidity come from here? NULL for onramping
-- 	<ms ts>,			the ms timestamp we use here
-- 	8000,			the compensation if the maker is incorrect.
-- 	NULL,			was this maker correct in the end? NULL if it hasn't happened yet
-- 	NULL,			the timestamplet id if we're at the point where we needed it
-- 	NULL			the server signature if the aggregate happened for this taker + maker combo
-- );

-- Where we would run the auction until we're not able to anymore, at which point we would
-- choose the winner. We would update the table accordingly.

CREATE VIEW rfqhub_bundle_taker_amt_1 AS
	SELECT acc_addr, SUM(amt) AS amt, asset
	FROM rfqhub_bundles_taker_1
	WHERE

-- Create the bundle that can be used later by the makers:

CREATE FUNCTION rfqhub_create_bundle_taker_onramping_1 (
	acc_addr ADDRESS,
	asset ADDRESS,
	min_bundle_amt INTEGER,
	max_bundle_amt INTEGER,
	is_up BOOLEAN,
	price_target BIGINT,
	outcome RFQHUB_OUTCOME,
	ms_ts INTEGER,
	expiry TIMESTAMP,
	user_sig BYTES64
)
RETURNS BIGINT
LANGUAGE plpgsql
AS $$
BEGIN;
END;

-- This function is when someone submits a BundleMaker that may or may not be chosen in the end.

CREATE FUNCTION rfqhub_create_bundle_maker_onramping_1 (
	acc_addr ADDRESS,
	taker_bundle_id INTEGER,
	min_bundle_amt INTEGER,
	max_bundle_amt INTEGER,
	ms_ts INTEGER
)

-- migrate:down
