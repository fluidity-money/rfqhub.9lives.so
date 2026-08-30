#[cfg(feature = "proptest")]
mod test {
    use proptest::prelude::*;
    use superposition_librfq::{
        AggregateEnum, OnrampingSource,
        strategies::{InvertedAggregate, PriceDirection, inverted_aggregate},
    };

    fn as_u64(value: [u8; 6]) -> u64 {
        let mut bytes = [0u8; 8];
        bytes[2..].copy_from_slice(&value);
        u64::from_be_bytes(bytes)
    }

    fn assert_monotonic(trace: &InvertedAggregate) {
        let submissions = trace.submissions();
        let mut previous_expiry = None;

        for submission in submissions {
            let taker_ms_ts = as_u64(submission.taker_ms_ts);
            let maker_ms_ts = as_u64(submission.maker_ms_ts);
            let submitted_at = as_u64(submission.submitted_at);
            let expiry = as_u64(submission.expiry);

            assert!(taker_ms_ts < maker_ms_ts);
            assert!(maker_ms_ts <= submitted_at);
            assert!(submitted_at < expiry);
            if let Some(previous) = previous_expiry {
                assert!(previous < taker_ms_ts);
            }
            previous_expiry = Some(expiry);
        }
    }

    fn assert_expected_returns(trace: &InvertedAggregate) {
        let mut available = trace.initial_taker.amt;
        for submission in trace.submissions() {
            let did_taker_win = match submission.price_move.direction {
                PriceDirection::Up => submission.timestamplet_val > submission.price_target,
                PriceDirection::Down => submission.timestamplet_val <= submission.price_target,
            };
            available = if did_taker_win {
                available.checked_add(submission.maker_amt).unwrap()
            } else {
                0
            };
            assert_eq!(submission.price_move.expected_return, available);
        }
    }

    fn assert_conversion(trace: &InvertedAggregate) {
        let submissions = trace.submissions();
        let mut converted = trace.to_aggregate_enum();

        for expected in submissions.into_iter().rev() {
            let AggregateEnum::Args(aggregate) = converted else {
                panic!("inverted traces must convert to inline aggregate arguments");
            };
            let actual = aggregate.args.maker.args;
            assert_eq!(actual.amt, expected.maker_amt);
            assert_eq!(actual.ms_ts, expected.maker_ms_ts);
            assert_eq!(actual.taker.args.expiry, expected.expiry);
            assert_eq!(actual.taker.args.ms_ts, expected.taker_ms_ts);
            assert_eq!(
                actual.taker.args.is_up,
                expected.price_move.direction.is_up()
            );
            assert_eq!(
                aggregate.args.timestamplet.timestamplet_time,
                expected.expiry
            );
            assert_eq!(
                aggregate.args.timestamplet.timestamplet_val,
                expected.timestamplet_val
            );

            converted = match actual.taker.args.src {
                OnrampingSource::AggregatedResult(previous) => *previous,
                OnrampingSource::OnchainPot(pot) => {
                    assert_eq!(pot, trace.initial_taker);
                    return;
                }
                OnrampingSource::OnchainPotAggregatedResult(_, _) => {
                    panic!("the trace converter must produce a linear aggregate chain")
                }
            };
        }
        panic!("conversion ended before reaching its initial pot");
    }

    proptest! {
        #[test]
        fn generated_inverted_aggregates_are_valid(trace in inverted_aggregate()) {
            assert_monotonic(&trace);
            assert_expected_returns(&trace);
            assert_conversion(&trace);
        }
    }
}
