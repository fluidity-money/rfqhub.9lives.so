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

CREATE TABLE rfqhub_bundle_user_count_1 (
	id SERIAL PRIMARY KEY,
	addr ADDRESS NOT NULL UNIQUE,
	count INTEGER NOT NULL DEFAULT 0
);

-- BundleMaker bundles that are made after validation on the server side.
CREATE TABLE rfqhub_bundle_maker_1 (
	id SERIAL PRIMARY KEY,
	created_by TIMESTAMP NOT NULL,
	-- Count of the bundle per user derived from rfqhub_bundle_user_count_1.
	user_id INTEGER NOT NULL UNIQUE
	asset ADDRESS NOT NULL,
	user_addr ADDRESS NOT NULL
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
