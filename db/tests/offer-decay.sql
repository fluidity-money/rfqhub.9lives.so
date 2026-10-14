-- Disposable PostgreSQL: psql -X -v ON_ERROR_STOP=1 -f db/tests/offer-decay.sql
BEGIN;
SET LOCAL TIME ZONE 'UTC';
SET LOCAL plpgsql.check_asserts = on;
CREATE TEMP TABLE rfqhub_events_balance_changed (
 id BIGSERIAL PRIMARY KEY, account_addr ADDRESS, asset ADDRESS, amount BIGINT
);
CREATE TEMP VIEW rfqhub_onramped_balances_1 AS
SELECT account_addr, asset, SUM(amount) AS amount
FROM rfqhub_events_balance_changed GROUP BY account_addr, asset;

DO $$
DECLARE
 taker ADDRESS := '0x00000000000000000000000000000000000000a1';
 maker ADDRESS := '0x00000000000000000000000000000000000000b1';
 asset_addr ADDRESS := '0x00000000000000000000000000000000000000c1';
 target INTEGER;
 r RECORD;
 c RECORD;
 deadline INTEGER := EXTRACT(EPOCH FROM CURRENT_TIMESTAMP + INTERVAL '1 hour')::INTEGER;
 expiry INTEGER := EXTRACT(EPOCH FROM CURRENT_TIMESTAMP + INTERVAL '2 hours')::INTEGER;
BEGIN
 INSERT INTO rfqhub_events_balance_changed(account_addr,asset,amount)
 VALUES(taker,asset_addr,10000),(maker,asset_addr,10000);
 SELECT * INTO STRICT r FROM rfqhub_create_bundle_taker_1(
  taker,asset_addr,100,100,TRUE,100,'BitcoinPrice',1,expiry,NULL,deadline,0.1);
 target := r.bundle_taker_id;
 ASSERT (SELECT min_offer IS NULL AND decay_rate=0.1 FROM rfqhub_bundles_taker_1 WHERE id=target);
 PERFORM rfqhub_set_bundle_taker_structure_1(target,'{"test":true}');

 -- Each case uses transaction time, so boundaries are deterministic without sleeps.
 FOR c IN SELECT * FROM (VALUES
  (NULL::BIGINT,0.1::NUMERIC,0::NUMERIC,1::BIGINT,TRUE),
  (100,0,10,100,FALSE),
  (100,0,10,101,TRUE),
  (100,0.1,0,100,FALSE),
  (100,0.1,0.9,100,FALSE),
  (100,0.1,1,90,FALSE),
  (100,0.1,1,91,TRUE),
  (100,0.1,2,81,FALSE),
  (100,0.1,2,82,TRUE),
  (100,0.1,3,72,FALSE),
  (100,0.1,3,73,TRUE),
  (100,1,0,100,FALSE),
  (100,1,1,1,TRUE)
 ) AS cases(minimum,rate,seconds,offer,accepted)
 LOOP
  UPDATE rfqhub_bundles_taker_1
  SET min_offer=c.minimum,decay_rate=c.rate,
   created_by=CURRENT_TIMESTAMP - c.seconds * INTERVAL '1 second'
  WHERE id=target;
  BEGIN
   PERFORM * FROM rfqhub_create_bundle_maker_1(maker,target,c.offer,c.offer,1);
   ASSERT c.accepted, 'accepted an offer at or below the decayed minimum';
  EXCEPTION WHEN raise_exception THEN
   IF SQLERRM NOT LIKE '%must exceed taker minimum offer%' THEN RAISE; END IF;
   ASSERT NOT c.accepted, 'rejected an offer above the decayed minimum';
  END;
 END LOOP;

 FOR c IN SELECT * FROM (VALUES (-0.1::NUMERIC),(1.1::NUMERIC),('NaN'::NUMERIC),(NULL::NUMERIC)) AS rates(rate)
 LOOP
  BEGIN
   PERFORM * FROM rfqhub_create_bundle_taker_1(
    taker,asset_addr,100,100,TRUE,100,'BitcoinPrice',1,expiry,NULL,deadline,c.rate);
   ASSERT FALSE, 'accepted invalid decay rate';
  EXCEPTION WHEN raise_exception THEN
   IF SQLERRM NOT LIKE '%decay rate must be between zero and one%' THEN RAISE; END IF;
  END;
 END LOOP;

 UPDATE rfqhub_bundles_taker_1 SET deadline=CURRENT_TIMESTAMP WHERE id=target;
 BEGIN
  PERFORM * FROM rfqhub_create_bundle_maker_1(maker,target,1,1,1);
  ASSERT FALSE, 'accepted offer at deadline';
 EXCEPTION WHEN raise_exception THEN
  IF SQLERRM NOT LIKE '%deadline has passed%' THEN RAISE; END IF;
 END;
END;
$$;
ROLLBACK;
