-- Disposable PostgreSQL only: psql -X -v ON_ERROR_STOP=1 -f db/tests/maker-multi-source.sql
-- Everything, including helpers and event fixtures, rolls back.
BEGIN;
SET LOCAL plpgsql.check_asserts = on;
CREATE TEMP TABLE rfqhub_events_balance_changed (
 id BIGSERIAL PRIMARY KEY, account_addr ADDRESS, asset ADDRESS, amount BIGINT
);
CREATE TEMP VIEW rfqhub_onramped_balances_1 AS
SELECT account_addr, asset, SUM(amount) AS amount
FROM rfqhub_events_balance_changed GROUP BY account_addr, asset;

CREATE FUNCTION pg_temp.maker_test_target(p_account ADDRESS, p_asset ADDRESS)
RETURNS INTEGER LANGUAGE plpgsql AS $$
DECLARE v_id INTEGER; v_user INTEGER;
BEGIN
 INSERT INTO rfqhub_bundles_taker_user_count_1 AS c (addr,count)
 VALUES (p_account,1) ON CONFLICT (addr) DO UPDATE SET count=c.count+1
 RETURNING count INTO v_user;
 INSERT INTO rfqhub_bundles_taker_1
 (acc_addr,user_id,asset,amt,min_offer,is_up,price_target,outcome,ms_ts,deadline,expiry,structure)
 VALUES (p_account,v_user,p_asset,1,0,TRUE,100,'BitcoinPrice',1,
 CURRENT_TIMESTAMP + INTERVAL '1 hour',CURRENT_TIMESTAMP + INTERVAL '2 hours','{"target":true}')
 RETURNING id INTO v_id;
 RETURN v_id;
END;
$$;
CREATE FUNCTION pg_temp.maker_test_winner(p_account ADDRESS, p_asset ADDRESS, p_amt BIGINT)
RETURNS INTEGER LANGUAGE plpgsql AS $$
DECLARE v_id INTEGER; v_user INTEGER;
BEGIN
 v_id := pg_temp.maker_test_target(p_account,p_asset);
 UPDATE rfqhub_bundles_taker_1 SET deadline='2025-12-31',expiry='2026-01-01' WHERE id=v_id;
 INSERT INTO rfqhub_bundles_maker_user_count_1 AS c (addr,count)
 VALUES ('0x00000000000000000000000000000000000000c1',1)
 ON CONFLICT (addr) DO UPDATE SET count=c.count+1 RETURNING count INTO v_user;
 INSERT INTO rfqhub_bundles_maker_1 (acc_addr,user_id,bundle_taker_id,ms_ts,amt,structure)
 VALUES ('0x00000000000000000000000000000000000000c1',v_user,v_id,1,p_amt-1,'{"prior":true}');
 PERFORM * FROM rfqhub_conclude_aggregate_1(v_id,200);
 RETURN v_id;
END;
$$;
DO $$
DECLARE
 a ADDRESS := '0x00000000000000000000000000000000000000a1';
 b ADDRESS := '0x00000000000000000000000000000000000000b1';
 other ADDRESS := '0x00000000000000000000000000000000000000d1';
 target INTEGER; big INTEGER; small1 INTEGER; small2 INTEGER; src INTEGER;
 offer INTEGER; chosen INTEGER; counter INTEGER; n INTEGER; rows INTEGER;
 ids INTEGER[]; amounts BIGINT[]; r RECORD;
 deadline INTEGER := EXTRACT(EPOCH FROM CURRENT_TIMESTAMP + INTERVAL '1 hour')::INTEGER;
 expiry INTEGER := EXTRACT(EPOCH FROM CURRENT_TIMESTAMP + INTERVAL '2 hours')::INTEGER;
BEGIN
 INSERT INTO rfqhub_events_balance_changed(account_addr,asset,amount) VALUES(a,b,100),(other,b,100);
 target := pg_temp.maker_test_target(other,b);
 big := pg_temp.maker_test_winner(a,b,30);
 small1 := pg_temp.maker_test_winner(a,b,10);
 small2 := pg_temp.maker_test_winner(a,b,10);
 -- Aggregate only: tie-break by ID, partial final source reserves entire result.
 FOR r IN SELECT * FROM rfqhub_create_bundle_maker_1(a,target,25,25,1) LOOP
  offer := r.bundle_maker_id;
  ids := array_append(ids,(SELECT bundle_taker_id FROM rfqhub_bundles_maker_1 WHERE id=r.source_maker_id));
  amounts := array_append(amounts,r.source_amt);
  ASSERT r.maker_amt=25 AND r.taker_outcome='BitcoinPrice';
  ASSERT r.structure_bundle_taker::JSONB='{"target":true}'::JSONB;
  ASSERT r.bundle_maker_structure::JSONB='{"prior":true}'::JSONB;
  ASSERT r.timestamplet_id IS NOT NULL AND r.timestamplet_end_value=200;
  ASSERT r.timestamplet_time='2026-01-01'::TIMESTAMP AND r.timestamplet_outcome='BitcoinPrice';
 END LOOP;
 ASSERT ids=ARRAY[small1,small2,big] AND amounts=ARRAY[10,10,5]::BIGINT[];
 ASSERT (SELECT pot_funded_amt=0 AND amt=25 FROM rfqhub_bundles_maker_1 WHERE id=offer);
 ASSERT (SELECT amt_to_onramp_amt=25 FROM rfqhub_bundles_taker_sources_1 WHERE source_taker_id=big);
 ASSERT rfqhub_get_available_balance_1(a,b)=100;
 BEGIN
  PERFORM * FROM rfqhub_mark_winner_as_offramped_1(a,TRUE,big);
  RAISE EXCEPTION 'withdrawal accepted reserved maker source';
 EXCEPTION WHEN raise_exception THEN
  IF SQLERRM NOT LIKE '%already consumed%' THEN RAISE; END IF;
 END;
 -- Compatibility with installations that retain the legacy taker constructor.
 -- The current user-edited migration removed its definition; do not restore it
 -- implicitly. The shared reservation function and active index protect either role.
 IF EXISTS (SELECT 1 FROM pg_proc WHERE proname='rfqhub_create_bundle_taker_from_aggregate_1') THEN
  BEGIN
   PERFORM * FROM rfqhub_create_bundle_taker_from_aggregate_1(big,1,TRUE,100,'BitcoinPrice',1,expiry,0,deadline);
   RAISE EXCEPTION 'legacy accepted maker-reserved source';
  EXCEPTION WHEN raise_exception THEN
   IF SQLERRM NOT LIKE '%already consumed%' THEN RAISE; END IF;
  END;
 END IF;
 -- A taker constructor cannot use sources owned by the new maker.
 SELECT * INTO STRICT r FROM rfqhub_create_bundle_taker_1(a,b,100,100,TRUE,100,'BitcoinPrice',1,expiry,0,deadline);
 ASSERT r.source_maker_id IS NULL AND r.source_amt=100;
 UPDATE rfqhub_bundles_taker_1 SET deadline=CURRENT_TIMESTAMP WHERE id=r.bundle_taker_id;
 ASSERT rfqhub_get_spendable_1(a,b)=100;
 -- Neither an unsigned/unselected offer nor a selected one releases on expiry.
 UPDATE rfqhub_bundles_taker_1 SET deadline=CURRENT_TIMESTAMP WHERE id=target;
 ASSERT rfqhub_get_available_balance_1(a,b)=100;
 PERFORM rfqhub_pick_maker_1(offer);
 ASSERT rfqhub_taker_source_reserved_1(big);
 ASSERT rfqhub_get_available_balance_1(a,b)=100;

 -- Mixed funding; pot reservation is 20, not total 40.
 target := pg_temp.maker_test_target(other,b);
 src := pg_temp.maker_test_winner(a,b,20);
 amounts := NULL;
 FOR r IN SELECT * FROM rfqhub_create_bundle_maker_1(a,target,40,40,1) LOOP
  offer := r.bundle_maker_id;
  amounts := array_append(amounts,r.source_amt);
  ASSERT r.maker_amt=40;
 END LOOP;
 ASSERT amounts=ARRAY[20,20]::BIGINT[] AND r.source_maker_id IS NULL;
 ASSERT r.bundle_maker_structure IS NULL AND r.timestamplet_id IS NULL;
 ASSERT r.timestamplet_end_value IS NULL AND r.timestamplet_time IS NULL AND r.timestamplet_outcome IS NULL;
 ASSERT (SELECT pot_funded_amt=20 FROM rfqhub_bundles_maker_1 WHERE id=offer);
 ASSERT rfqhub_get_spendable_1(a,b)=80 AND rfqhub_get_available_balance_1(a,b)=80;
 -- Pot-only second offer, then definitive rejection restores aggregate and pot.
 SELECT * INTO STRICT r FROM rfqhub_create_bundle_maker_1(a,target,5,5,1);
 chosen := r.bundle_maker_id;
 ASSERT r.source_maker_id IS NULL AND r.source_amt=5 AND r.maker_amt=5;
 PERFORM rfqhub_pick_maker_1(chosen);
 ASSERT rfqhub_get_spendable_1(a,b)=95 AND rfqhub_get_available_balance_1(a,b)=115;
 ASSERT (SELECT released_at IS NOT NULL AND source_amt=20 AND amt_to_onramp_amt=0
  FROM rfqhub_bundles_taker_sources_1 WHERE bundle_maker_id=offer);
 ASSERT (SELECT end_source_amt=20 FROM rfqhub_bundles_taker_1 WHERE id=src);
 -- A taker can now reserve exactly the released source; history remains intact.
 SELECT * INTO STRICT r FROM rfqhub_create_bundle_taker_1(a,b,10,10,TRUE,100,'BitcoinPrice',1,expiry,0,deadline);
 ASSERT r.source_maker_id IS NOT NULL AND r.source_amt=10;
 ASSERT (SELECT count(*)=2 FROM rfqhub_bundles_taker_sources_1 WHERE source_taker_id=src);
 target := pg_temp.maker_test_target(other,b);
 SELECT * INTO STRICT r FROM rfqhub_create_bundle_maker_1(a,target,95,95,1);
 ASSERT r.source_maker_id IS NULL AND r.source_amt=95;

 -- Minimum failure rolls back lazy release, counters, and all funding writes.
 src := pg_temp.maker_test_winner(a,b,7);
 SELECT * INTO STRICT r FROM rfqhub_create_bundle_taker_1(a,b,7,7,TRUE,100,'BitcoinPrice',1,expiry,0,deadline);
 UPDATE rfqhub_bundles_taker_1 SET deadline=CURRENT_TIMESTAMP WHERE id=r.bundle_taker_id;
 SELECT count(*) INTO n FROM rfqhub_bundles_maker_1;
 SELECT count INTO counter FROM rfqhub_bundles_maker_user_count_1 WHERE addr=a;
 BEGIN
  PERFORM * FROM rfqhub_create_bundle_maker_1(a,target,8,8,1);
  RAISE EXCEPTION 'minimum unexpectedly accepted';
 EXCEPTION WHEN raise_exception THEN
  IF SQLERRM NOT LIKE 'insufficient maker funding:%' THEN RAISE; END IF;
 END;
 ASSERT (SELECT count(*)=n FROM rfqhub_bundles_maker_1);
 ASSERT (SELECT count=counter FROM rfqhub_bundles_maker_user_count_1 WHERE addr=a);
 ASSERT (SELECT NOT sources_released FROM rfqhub_bundles_taker_1 WHERE id=r.bundle_taker_id);
 ASSERT (SELECT released_at IS NULL FROM rfqhub_bundles_taker_sources_1 WHERE source_taker_id=src);
 -- Withdrawal can claim the logically expired source, but not its next consumer.
 PERFORM * FROM rfqhub_mark_winner_as_offramped_1(a,TRUE,src);
 ASSERT rfqhub_get_available_balance_1(a,b)=0;

 -- Reject foreign, wrong-asset, unsigned, unresolved, withdrawn, maker-side results.
 PERFORM pg_temp.maker_test_winner(other,b,9);
 PERFORM pg_temp.maker_test_winner(a,'0x00000000000000000000000000000000000000b2',9);
 src := pg_temp.maker_test_winner(a,b,9);
 UPDATE rfqhub_bundles_taker_1 SET structure=NULL WHERE id=src;
 src := pg_temp.maker_test_winner(a,b,9);
 UPDATE rfqhub_bundles_maker_1 SET structure=NULL WHERE bundle_taker_id=src;
 src := pg_temp.maker_test_winner(a,b,9);
 UPDATE rfqhub_bundles_maker_1 SET timestamplet_id=NULL WHERE bundle_taker_id=src;
 src := pg_temp.maker_test_winner(a,b,9);
 UPDATE rfqhub_bundles_maker_1 SET was_chosen=FALSE WHERE bundle_taker_id=src;
 src := pg_temp.maker_test_winner(a,b,9);
 UPDATE rfqhub_bundles_maker_1 SET was_correct=TRUE,end_source_amt=9 WHERE bundle_taker_id=src;
 UPDATE rfqhub_bundles_taker_1 SET end_source_amt=0 WHERE id=src;
 -- Legacy taker -> maker exclusion too, when compatibility function exists.
 IF EXISTS (SELECT 1 FROM pg_proc WHERE proname='rfqhub_create_bundle_taker_from_aggregate_1') THEN
  src := pg_temp.maker_test_winner(a,b,11);
  PERFORM * FROM rfqhub_create_bundle_taker_from_aggregate_1(src,1,TRUE,100,'BitcoinPrice',1,expiry,0,deadline);
 END IF;
 ASSERT rfqhub_get_available_balance_1(a,b)=0;
 BEGIN
  PERFORM * FROM rfqhub_create_bundle_maker_1(a,target,1,1,1);
  RAISE EXCEPTION 'ineligible source accepted';
 EXCEPTION WHEN raise_exception THEN
  IF SQLERRM NOT LIKE 'insufficient maker funding:%' THEN RAISE; END IF;
 END;
END;
$$;
ROLLBACK;
