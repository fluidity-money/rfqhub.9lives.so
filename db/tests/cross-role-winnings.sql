-- Disposable migrated PostgreSQL only:
-- psql -X -v ON_ERROR_STOP=1 -f db/tests/cross-role-winnings.sql
-- Fixtures, helper functions and constructor writes all roll back.
BEGIN;
SET LOCAL plpgsql.check_asserts = on;
CREATE TEMP TABLE rfqhub_events_balance_changed (
 id BIGSERIAL PRIMARY KEY, account_addr ADDRESS, asset ADDRESS, amount BIGINT
);
CREATE TEMP VIEW rfqhub_onramped_balances_1 AS
SELECT account_addr, asset, SUM(amount) AS amount
FROM rfqhub_events_balance_changed GROUP BY account_addr, asset;

CREATE FUNCTION pg_temp.cross_target(a ADDRESS, b ADDRESS)
RETURNS INTEGER LANGUAGE plpgsql AS $$
DECLARE i INTEGER; u INTEGER;
BEGIN
 INSERT INTO rfqhub_bundles_taker_user_count_1 AS c(addr,count) VALUES(a,1)
 ON CONFLICT(addr) DO UPDATE SET count=c.count+1 RETURNING count INTO u;
 INSERT INTO rfqhub_bundles_taker_1
 (acc_addr,user_id,asset,amt,min_offer,is_up,price_target,outcome,ms_ts,deadline,expiry,structure)
 VALUES(a,u,b,1,0,TRUE,100,'BitcoinPrice',1,
 CURRENT_TIMESTAMP+INTERVAL '1 hour',CURRENT_TIMESTAMP+INTERVAL '2 hours','{"target":true}')
 RETURNING id INTO i;
 RETURN i;
END;
$$;
CREATE FUNCTION pg_temp.cross_winner(a ADDRESS,b ADDRESS,n BIGINT,maker_wins BOOLEAN)
RETURNS INTEGER LANGUAGE plpgsql AS $$
DECLARE i INTEGER; u INTEGER; loser ADDRESS := '0x00000000000000000000000000000000000000f0';
 owner ADDRESS;
BEGIN
 i := pg_temp.cross_target(CASE WHEN maker_wins THEN loser ELSE a END,b);
 UPDATE rfqhub_bundles_taker_1 SET deadline='2025-12-31',expiry='2026-01-01',
 is_up=NOT maker_wins WHERE id=i;
 owner := CASE WHEN maker_wins THEN a ELSE loser END;
 INSERT INTO rfqhub_bundles_maker_user_count_1 AS c(addr,count) VALUES(owner,1)
 ON CONFLICT(addr) DO UPDATE SET count=c.count+1 RETURNING count INTO u;
 INSERT INTO rfqhub_bundles_maker_1(acc_addr,user_id,bundle_taker_id,ms_ts,amt,structure)
 VALUES(owner,u,i,1,n-1,'{"prior":true}');
 -- Actual conclusion sets selected maker, winner amounts and timestamplet.
 PERFORM * FROM rfqhub_conclude_aggregate_1(i,200);
 ASSERT (SELECT m.was_correct=maker_wins FROM rfqhub_bundles_maker_1 m
  JOIN rfqhub_bundles_taker_1 t ON t.maker_id=m.id WHERE t.id=i);
 -- Losing-side withdrawal flags must not govern eligibility or lazy release.
 IF maker_wins THEN
  UPDATE rfqhub_bundles_taker_1 SET was_offramped=TRUE WHERE id=i;
 ELSE
  UPDATE rfqhub_bundles_maker_1 SET was_offramped=TRUE WHERE bundle_taker_id=i;
 END IF;
 RETURN i;
END;
$$;
DO $$
DECLARE
 a ADDRESS := '0x00000000000000000000000000000000000000f1';
 b ADDRESS := '0x00000000000000000000000000000000000000f2';
 other ADDRESS := '0x00000000000000000000000000000000000000f3';
 prior_maker BOOLEAN; new_maker BOOLEAN; src INTEGER; target INTEGER;
 consumer INTEGER; rival INTEGER; r RECORD; before_count BIGINT;
 deadline INTEGER := EXTRACT(EPOCH FROM CURRENT_TIMESTAMP+INTERVAL '1 hour')::INTEGER;
 expiry INTEGER := EXTRACT(EPOCH FROM CURRENT_TIMESTAMP+INTERVAL '2 hours')::INTEGER;
BEGIN
 FOREACH prior_maker IN ARRAY ARRAY[FALSE,TRUE] LOOP
  FOREACH new_maker IN ARRAY ARRAY[FALSE,TRUE] LOOP
   src := pg_temp.cross_winner(a,b,30,prior_maker);
   target := pg_temp.cross_target(other,b);
   ASSERT rfqhub_get_available_balance_1(a,b)=30;
   ASSERT rfqhub_get_spendable_1(a,b)=0;
   IF new_maker THEN
    SELECT * INTO STRICT r FROM rfqhub_create_bundle_maker_1(a,target,12,12,1);
    consumer := r.bundle_maker_id;
   ELSE
    SELECT * INTO STRICT r FROM rfqhub_create_bundle_taker_1(a,b,12,12,TRUE,100,'BitcoinPrice',1,expiry,0,deadline);
    consumer := r.bundle_taker_id;
   END IF;
   ASSERT r.source_amt=12 AND r.bundle_maker_structure::JSONB='{"prior":true}'::JSONB;
   ASSERT r.timestamplet_end_value=200 AND r.timestamplet_time='2026-01-01'::TIMESTAMP;
   ASSERT r.timestamplet_outcome='BitcoinPrice';
   ASSERT (SELECT source_amt=12 AND amt_to_onramp_amt=18 FROM rfqhub_bundles_taker_sources_1
    WHERE source_taker_id=src AND released_at IS NULL);
   ASSERT rfqhub_get_available_balance_1(a,b)=0, 'partial source remainder is not local credit';
   BEGIN
    PERFORM * FROM rfqhub_mark_winner_as_offramped_1(a,NOT prior_maker,src);
    RAISE EXCEPTION 'reserved withdrawal accepted';
   EXCEPTION WHEN raise_exception THEN
    IF SQLERRM NOT LIKE '%already consumed%' THEN RAISE; END IF;
   END;
   -- Both consumer roles must be blocked from double-spending.
   BEGIN
    PERFORM * FROM rfqhub_create_bundle_taker_1(a,b,1,1,TRUE,100,'BitcoinPrice',1,expiry,0,deadline);
    RAISE EXCEPTION 'duplicate taker accepted';
   EXCEPTION WHEN raise_exception THEN
    IF SQLERRM NOT LIKE 'insufficient taker funding:%' THEN RAISE; END IF;
   END;
   BEGIN
    PERFORM * FROM rfqhub_create_bundle_maker_1(a,target,1,1,1);
    RAISE EXCEPTION 'duplicate maker accepted';
   EXCEPTION WHEN raise_exception THEN
    IF SQLERRM NOT LIKE 'insufficient maker funding:%' THEN RAISE; END IF;
   END;
   IF new_maker THEN
    -- Expiry alone never releases maker offers, even unsigned ones.
    UPDATE rfqhub_bundles_taker_1 SET deadline=CURRENT_TIMESTAMP WHERE id=target;
    ASSERT rfqhub_get_available_balance_1(a,b)=0;
    -- Fixture rival is definitively selected, releasing the rejected offer.
    INSERT INTO rfqhub_bundles_maker_1(acc_addr,user_id,bundle_taker_id,ms_ts,amt,structure)
    VALUES(other,target,target,1,1,'{"rival":true}') RETURNING id INTO rival;
    PERFORM rfqhub_pick_maker_1(rival);
    ASSERT (SELECT released_at IS NOT NULL FROM rfqhub_bundles_taker_sources_1
     WHERE source_taker_id=src AND bundle_maker_id=consumer);
   ELSE
    UPDATE rfqhub_bundles_taker_1 SET deadline=CURRENT_TIMESTAMP WHERE id=consumer;
   END IF;
   ASSERT rfqhub_get_available_balance_1(a,b)=30;
   -- A failed minimum must roll back lazy release and new reservations.
   SELECT count(*) INTO before_count FROM rfqhub_bundles_taker_sources_1;
   BEGIN
    PERFORM * FROM rfqhub_create_bundle_taker_1(a,b,31,31,TRUE,100,'BitcoinPrice',1,expiry,0,deadline);
    RAISE EXCEPTION 'minimum accepted';
   EXCEPTION WHEN raise_exception THEN
    IF SQLERRM NOT LIKE 'insufficient taker funding:%' THEN RAISE; END IF;
   END;
   ASSERT (SELECT count(*)=before_count FROM rfqhub_bundles_taker_sources_1);
   IF NOT new_maker THEN
    ASSERT (SELECT released_at IS NULL FROM rfqhub_bundles_taker_sources_1
     WHERE source_taker_id=src AND bundle_taker_id=consumer);
   END IF;
   -- Reuse after release, then withdraw after that consumer expires.
   SELECT * INTO STRICT r FROM rfqhub_create_bundle_taker_1(a,b,10,10,TRUE,100,'BitcoinPrice',1,expiry,0,deadline);
   ASSERT (SELECT count(*)=2 FROM rfqhub_bundles_taker_sources_1 WHERE source_taker_id=src);
   BEGIN
    UPDATE rfqhub_bundles_taker_sources_1 SET released_at=NULL WHERE source_taker_id=src;
    RAISE EXCEPTION 'duplicate active key accepted';
   EXCEPTION WHEN unique_violation THEN NULL;
   END;
   UPDATE rfqhub_bundles_taker_1 SET deadline=CURRENT_TIMESTAMP WHERE id=r.bundle_taker_id;
   PERFORM * FROM rfqhub_mark_winner_as_offramped_1(a,NOT prior_maker,src);
   ASSERT NOT rfqhub_taker_source_reserved_1(src);
   ASSERT rfqhub_get_available_balance_1(a,b)=0;
   ASSERT rfqhub_get_spendable_1(a,b)=0;
   RAISE NOTICE 'PASS prior maker %, new maker %',prior_maker,new_maker;
  END LOOP;
 END LOOP;
END;
$$;
-- Mixed winning sides: global smallest-first amount/parent-ID ordering, pot last.
DO $$
DECLARE
 a ADDRESS := '0x00000000000000000000000000000000000000f4';
 b ADDRESS := '0x00000000000000000000000000000000000000f2';
 other ADDRESS := '0x00000000000000000000000000000000000000f3';
 small_m INTEGER; small_t INTEGER; big INTEGER; target INTEGER; r RECORD;
 ids INTEGER[]; amounts BIGINT[]; role BOOLEAN;
 deadline INTEGER := EXTRACT(EPOCH FROM CURRENT_TIMESTAMP+INTERVAL '1 hour')::INTEGER;
 expiry INTEGER := EXTRACT(EPOCH FROM CURRENT_TIMESTAMP+INTERVAL '2 hours')::INTEGER;
BEGIN
 FOREACH role IN ARRAY ARRAY[FALSE,TRUE] LOOP
  small_m := pg_temp.cross_winner(a,b,10,TRUE);
  big := pg_temp.cross_winner(a,b,30,TRUE);
  small_t := pg_temp.cross_winner(a,b,10,FALSE);
  INSERT INTO rfqhub_events_balance_changed(account_addr,asset,amount) VALUES(a,b,7);
  ids := NULL; amounts := NULL;
  IF role THEN
   target := pg_temp.cross_target(other,b);
   FOR r IN SELECT * FROM rfqhub_create_bundle_maker_1(a,target,57,57,1) LOOP
    ids := array_append(ids,(SELECT bundle_taker_id FROM rfqhub_bundles_maker_1 WHERE id=r.source_maker_id));
    amounts := array_append(amounts,r.source_amt);
    ASSERT r.maker_amt=57;
   END LOOP;
  ELSE
   FOR r IN SELECT * FROM rfqhub_create_bundle_taker_1(a,b,57,57,TRUE,100,'BitcoinPrice',1,expiry,0,deadline) LOOP
    ids := array_append(ids,(SELECT bundle_taker_id FROM rfqhub_bundles_maker_1 WHERE id=r.source_maker_id));
    amounts := array_append(amounts,r.source_amt);
    ASSERT r.amt=57;
   END LOOP;
  END IF;
  ASSERT ids IS NOT DISTINCT FROM ARRAY[small_m,small_t,big,NULL::INTEGER];
  ASSERT amounts=ARRAY[10,10,30,7]::BIGINT[];
  ASSERT r.source_maker_id IS NULL AND r.timestamplet_id IS NULL;
  ASSERT rfqhub_get_available_balance_1(a,b)=0 AND rfqhub_get_spendable_1(a,b)=0;
 END LOOP;
END;
$$;
-- Legacy maker.source uses selected-maker identity, not aggregate-parent ID.
DO $$
DECLARE
 a ADDRESS := '0x00000000000000000000000000000000000000f5';
 b ADDRESS := '0x00000000000000000000000000000000000000f2';
 other ADDRESS := '0x00000000000000000000000000000000000000f3';
 src INTEGER; target INTEGER; child INTEGER;
BEGIN
 src := pg_temp.cross_winner(a,b,20,TRUE);
 target := pg_temp.cross_target(other,b);
 INSERT INTO rfqhub_bundles_maker_1(acc_addr,user_id,bundle_taker_id,source,ms_ts,amt)
 SELECT a,10000,target,maker_id,1,10 FROM rfqhub_bundles_taker_1 WHERE id=src RETURNING id INTO child;
 ASSERT rfqhub_taker_source_reserved_1(src);
 ASSERT rfqhub_get_available_balance_1(a,b)=0;
 BEGIN
  PERFORM * FROM rfqhub_mark_winner_as_offramped_1(a,FALSE,src);
  RAISE EXCEPTION 'legacy reserved withdrawal accepted';
 EXCEPTION WHEN raise_exception THEN
  IF SQLERRM NOT LIKE '%already consumed%' THEN RAISE; END IF;
 END;
 UPDATE rfqhub_bundles_maker_1 SET was_chosen=FALSE WHERE id=child;
 ASSERT NOT rfqhub_taker_source_reserved_1(src);
 ASSERT rfqhub_get_available_balance_1(a,b)=20;
 PERFORM * FROM rfqhub_mark_winner_as_offramped_1(a,FALSE,src);
 ASSERT rfqhub_get_available_balance_1(a,b)=0;
END;
$$;
-- Maker-winner exclusions apply identically to balance and both constructors.
DO $$
DECLARE
 a ADDRESS := '0x00000000000000000000000000000000000000f6';
 b ADDRESS := '0x00000000000000000000000000000000000000f2';
 other ADDRESS := '0x00000000000000000000000000000000000000f3';
 src INTEGER; target INTEGER; kind TEXT;
 deadline INTEGER := EXTRACT(EPOCH FROM CURRENT_TIMESTAMP+INTERVAL '1 hour')::INTEGER;
 expiry INTEGER := EXTRACT(EPOCH FROM CURRENT_TIMESTAMP+INTERVAL '2 hours')::INTEGER;
BEGIN
 target := pg_temp.cross_target(other,b);
 FOREACH kind IN ARRAY ARRAY['unsigned taker','unsigned maker','offramped','zero','null',
  'unknown winner','unselected','no timestamplet','foreign account','foreign asset'] LOOP
  src := pg_temp.cross_winner(a,b,20,TRUE);
  CASE kind
   WHEN 'unsigned taker' THEN UPDATE rfqhub_bundles_taker_1 SET structure=NULL WHERE id=src;
   WHEN 'unsigned maker' THEN UPDATE rfqhub_bundles_maker_1 SET structure=NULL WHERE bundle_taker_id=src;
   WHEN 'offramped' THEN UPDATE rfqhub_bundles_maker_1 SET was_offramped=TRUE WHERE bundle_taker_id=src;
   WHEN 'zero' THEN UPDATE rfqhub_bundles_maker_1 SET end_source_amt=0 WHERE bundle_taker_id=src;
   WHEN 'null' THEN UPDATE rfqhub_bundles_maker_1 SET end_source_amt=NULL WHERE bundle_taker_id=src;
   WHEN 'unknown winner' THEN UPDATE rfqhub_bundles_maker_1 SET was_correct=NULL WHERE bundle_taker_id=src;
   WHEN 'unselected' THEN UPDATE rfqhub_bundles_maker_1 SET was_chosen=FALSE WHERE bundle_taker_id=src;
   WHEN 'no timestamplet' THEN UPDATE rfqhub_bundles_maker_1 SET timestamplet_id=NULL WHERE bundle_taker_id=src;
   WHEN 'foreign account' THEN
    -- Use a unique historical user ID for this fixture owner change.
    UPDATE rfqhub_bundles_maker_1 SET acc_addr=other,user_id=100000+src WHERE bundle_taker_id=src;
   WHEN 'foreign asset' THEN UPDATE rfqhub_bundles_taker_1 SET asset=other WHERE id=src;
  END CASE;
  ASSERT rfqhub_get_available_balance_1(a,b)=0, kind;
  BEGIN
   PERFORM * FROM rfqhub_create_bundle_taker_1(a,b,1,1,TRUE,100,'BitcoinPrice',1,expiry,0,deadline);
   RAISE EXCEPTION 'ineligible taker funding accepted';
  EXCEPTION WHEN raise_exception THEN
   IF SQLERRM NOT LIKE 'insufficient taker funding:%' THEN RAISE; END IF;
  END;
  BEGIN
   PERFORM * FROM rfqhub_create_bundle_maker_1(a,target,1,1,1);
   RAISE EXCEPTION 'ineligible maker funding accepted';
  EXCEPTION WHEN raise_exception THEN
   IF SQLERRM NOT LIKE 'insufficient maker funding:%' THEN RAISE; END IF;
  END;
 END LOOP;
END;
$$;
ROLLBACK;
