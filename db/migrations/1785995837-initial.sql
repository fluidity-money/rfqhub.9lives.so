-- migrate:up

-- This function subtracts the latest BalanceChanged(address,uint256)
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
-- follow the same user experience. So this is a type of situation where the
-- onramping amount is coloured:

-- digraph {
-- 	Onramping -> {
-- 		BundleMaker[xlabel="Market maker bundle"]
-- 		BundleTaker[xlabel="Taker bundle"]
-- 	}
-- }

-- So, ultimately the two can sort of pair each other out in the
-- recursive reconstruction of the original balance, where an edge worker
-- simulates the bundle's success if it were to be sent on-chain, and
-- then the bundles are used, and a new bundle is made with the two
-- combined together, and the concenated combination of the two is
-- created based on the inputs from the both, so that there's no race
-- issue here.

CREATE FUNCTION rfqhub_get_outstanding_balance(

-- migrate:down
