-- Disposable PostgreSQL: psql -X -v ON_ERROR_STOP=1 -f db/tests/offramp-pre.sql
BEGIN;
SET LOCAL plpgsql.check_asserts = on;
CREATE FUNCTION pg_temp.offramp_fixture(p_owner ADDRESS, p_asset ADDRESS, p_amount BIGINT, p_maker_wins BOOLEAN)
RETURNS INTEGER LANGUAGE plpgsql AS $$
DECLARE
 t INTEGER; u INTEGER;
 other ADDRESS := '0x00000000000000000000000000000000000000ee';
 taker ADDRESS := CASE WHEN p_maker_wins THEN other ELSE p_owner END;
 maker ADDRESS := CASE WHEN p_maker_wins THEN p_owner ELSE other END;
BEGIN
 INSERT INTO rfqhub_bundles_taker_user_count_1 AS c(addr,count)
 VALUES(taker,1) ON CONFLICT(addr) DO UPDATE SET count=c.count+1 RETURNING count INTO u;
 INSERT INTO rfqhub_bundles_taker_1
 (acc_addr,user_id,asset,amt,min_offer,is_up,price_target,outcome,ms_ts,deadline,expiry,structure)
 VALUES(taker,u,p_asset,1,0,NOT p_maker_wins,100,'BitcoinPrice',1,
 '2025-12-31','2026-01-01','{"taker":true}') RETURNING id INTO t;
 INSERT INTO rfqhub_bundles_maker_user_count_1 AS c(addr,count)
 VALUES(maker,1) ON CONFLICT(addr) DO UPDATE SET count=c.count+1 RETURNING count INTO u;
 INSERT INTO rfqhub_bundles_maker_1(acc_addr,user_id,bundle_taker_id,ms_ts,amt,structure)
 VALUES(maker,u,t,1,p_amount-1,'{"maker":true}');
 PERFORM rfqhub_conclude_aggregate_1(t,200);
 RETURN t;
END;
$$;
DO $$
DECLARE
 a ADDRESS := '0x00000000000000000000000000000000000000a1';
 asset ADDRESS := '0x00000000000000000000000000000000000000b1';
 other_asset ADDRESS := '0x00000000000000000000000000000000000000b2';
 small INTEGER; large INTEGER; excluded INTEGER; ids INTEGER[]; r RECORD;
BEGIN
 small := pg_temp.offramp_fixture(a,asset,10,FALSE);
 large := pg_temp.offramp_fixture(a,asset,20,TRUE);
 PERFORM pg_temp.offramp_fixture(a,other_asset,2,FALSE);
 excluded := pg_temp.offramp_fixture(a,asset,2,FALSE);
 UPDATE rfqhub_bundles_taker_1 SET was_offramped=TRUE WHERE id=excluded;
 excluded := pg_temp.offramp_fixture(a,asset,2,FALSE);
 UPDATE rfqhub_bundles_maker_1 SET structure=NULL WHERE bundle_taker_id=excluded;
 excluded := pg_temp.offramp_fixture(a,asset,2,TRUE);
 UPDATE rfqhub_bundles_maker_1 SET was_offramped=TRUE WHERE bundle_taker_id=excluded;

 BEGIN
  PERFORM * FROM rfqhub_offramp_pre_1(a,asset,31);
  ASSERT FALSE, 'insufficient balance must fail';
 EXCEPTION WHEN raise_exception THEN
  IF SQLERRM NOT LIKE '%insufficient offramp winnings%' THEN RAISE; END IF;
 END;
 ASSERT (SELECT NOT was_offramped FROM rfqhub_bundles_taker_1 WHERE id=small), 'failure must roll back taker mark';
 ASSERT (SELECT NOT was_offramped FROM rfqhub_bundles_maker_1 WHERE bundle_taker_id=large), 'failure must roll back maker mark';
 -- A selected signed offer without a timestamplet must not be eligible.
 excluded := pg_temp.offramp_fixture(a,asset,2,FALSE);
 UPDATE rfqhub_bundles_maker_1 SET timestamplet_id=NULL WHERE bundle_taker_id=excluded;
 FOR r IN SELECT * FROM rfqhub_offramp_pre_1(a,asset,11) LOOP
  ids := array_append(ids,r.bundle_taker_id);
  ASSERT r.bundle_maker_structure::JSONB='{"maker":true}'::JSONB;
  ASSERT r.timestamplet_end_value=200;
  ASSERT r.timestamplet_time='2026-01-01'::TIMESTAMP;
  ASSERT r.timestamplet_outcome='BitcoinPrice';
  ASSERT r.is_taker IS NOT DISTINCT FROM (r.bundle_taker_id=small), 'winner side must match requesting account';
  ASSERT (SELECT m.structure::JSONB=r.bundle_maker_structure::JSONB
   FROM rfqhub_bundles_taker_1 t JOIN rfqhub_bundles_maker_1 m ON m.id=t.maker_id
   WHERE t.id=r.bundle_taker_id);
 END LOOP;
 ASSERT ids=ARRAY[small,large], 'must cover amount with whole smallest-first winners from either side';
 ASSERT (SELECT was_offramped FROM rfqhub_bundles_taker_1 WHERE id=small), 'taker winner must be consumed';
 ASSERT (SELECT was_offramped FROM rfqhub_bundles_maker_1 WHERE bundle_taker_id=large), 'maker winner must be consumed';
 ASSERT (SELECT NOT was_offramped FROM rfqhub_bundles_maker_1 WHERE bundle_taker_id=small), 'losing maker must not be marked';
 ASSERT (SELECT NOT was_offramped FROM rfqhub_bundles_taker_1 WHERE id=large), 'losing taker must not be marked';
 BEGIN
  PERFORM * FROM rfqhub_offramp_pre_1(a,asset,1);
  ASSERT FALSE, 'consumed winners must not be returned again';
 EXCEPTION WHEN raise_exception THEN
  IF SQLERRM NOT LIKE '%insufficient offramp winnings%' THEN RAISE; END IF;
 END;
 BEGIN
  PERFORM * FROM rfqhub_offramp_pre_1(a,asset,0);
  ASSERT FALSE, 'zero request must fail';
 EXCEPTION WHEN raise_exception THEN
  IF SQLERRM NOT LIKE '%offramp amount must be positive%' THEN RAISE; END IF;
 END;
END;
$$;
ROLLBACK;
