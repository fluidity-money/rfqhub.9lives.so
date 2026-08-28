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

-- BundleTaker bundles are made after validation on the server side. This
-- table is a departure from our usual practice, in that it's updated
-- with whether the bundle is the latest status of it being used in the
-- recursive bundle, aka whether it's the definitive source in the taker.
CREATE TABLE rfqhub_bundles_taker_1 (
	id SERIAL PRIMARY KEY,
	created_by TIMESTAMP NOT NULL,
	-- Count of the bundle per user derived from rfqhub_bundle_user_count_1.
	user_id INTEGER NOT NULL UNIQUE,
	-- Is this BundleTaker used for an aggregate bundle?
	is_spendable BOOLEAN NOT NULL,
	-- Did this BundleTaker come from an onboarding source, or did it come
	-- from an aggregate bundle?
	from_res INTEGER,
	-- The asset is derived from the answer that we see after the aggregate is either
	-- submitted, or used in an aggregate bundle.
	asset ADDRESS NOT NULL,
	user_addr ADDRESS NOT NULL,
	-- What amount are we able to use with this BundleTaker here? Based on the end
	-- result of the aggregate, or the deposit.
	amt BIGINT NOT NULL
);

CREATE FUNCTION rfqhub_get_current_bundles_amt_1 (
	user_addr ADDRESS,
	asset ADDRESS
)
RETURNS BIGINT
LANGUAGE plpgsql
AS $$
BEGIN

END;

-- And the results of the outstanding bundles are used to subtract from this:

CREATE FUNCTION rfqhub_get_balance_1 (
	user_addr ADDRESS,
	asset ADDRESS
)
RETURNS BIGINT
LANGUAGE plpgsql
AS $$
DECLARE
	last_amt HUGEINT;
	open_bundles HUGEINT;
BEGIN

END:

-- And

CREATE FUNCTION rfqhub_create_bundle_1 (
	user_addr ADDRESS,
	asset ADDRESS,
	min_bundle_amt HUGEINT,
	max_bundle_amt HUGEINT
)

-- migrate:down
