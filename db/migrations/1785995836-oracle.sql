
-- migrate:up

CREATE TABLE oracles_ninelives_prices_3 (
	id SERIAL PRIMARY KEY,
	created_by TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
	base VARCHAR NOT NULL,
	quote VARCHAR NOT NULL,
	amount BIGINT NOT NULL,
	source VARCHAR NOT NULL
);

-- migrate:down
