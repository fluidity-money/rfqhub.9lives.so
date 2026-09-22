// Builds a deep recursive aggregate chain (many nested AggregateEnum::Args
// reached through OnrampingSource::AggregatedResult) and drives it through
// recurse_aggregate_args. The intent is to (a) prove the contract accepts a
// nested blob of N aggregate nodes, and (b) report the borsh-encoded calldata
// size so we can judge whether an N-deep blob is postable on-chain.
//
// Model: the taker starts from an on-chain pot of `initial_amt` and keeps
// reinvesting their full winnings into the next game. Every game is a taker
// win (is_up=true, timestamplet_val > price_target), so at each step the
// available funds grow by the maker's `maker_stake`.
use superposition_librfq::{entry_aggregate_to_offramp::*, signable::*, *};

extern crate alloc;

use alloc::boxed::Box;

use bobcat_sdk::{maths::U, storage::flush_guard_fresh};

/// Build a recursive AggregateEnum chain `depth` nodes deep.
///
/// The innermost taker draws from an OnchainPot; every outer taker draws from
/// the previous Aggregate via AggregatedResult. All nodes use the same taker
/// account and the same outcome/asset. Each maker stakes `maker_stake`.
///
/// Returns the built AggregateEnum alongside the amount `recurse_aggregate_args`
/// must return at the outermost node (the taker's claim after `depth` wins).
fn build_recursive_aggregate(
    depth: usize,
    initial_amt: u64,
    maker_stake: u64,
    taker: [u8; 20],
    makers: &[[u8; 20]],
) -> (AggregateEnum, u64) {
    let outcome = Outcome::BitcoinPrice;
    let asset = Asset::USDC;

    // Innermost taker source: funds come straight from the taker's pot.
    let mut source = OnrampingSource::OnchainPot(OnchainPotArgs {
        acc_addr: taker,
        amt: initial_amt,
        asset,
    });
    // Taker's running claim if they win every game.
    let mut available = initial_amt;

    for level in 0..depth {
        let maker_acc = makers[level];
        let aggregate = Aggregate {
            args: AggregateArgs {
                maker: BundleMaker {
                    args: BundleMakerArgs {
                        src: OnrampingSource::OnchainPot(OnchainPotArgs {
                            acc_addr: maker_acc,
                            amt: maker_stake,
                            asset,
                        }),
                        taker: BundleTaker {
                            args: BundleTakerArgs {
                                src: source,
                                expiry: (level + 1) as u32,
                                ms_ts: [(level as u8).wrapping_mul(2), 0, 0, 0, 0, 0],
                                is_up: true,
                                // Timestamplet val above target => taker always wins.
                                price_target: 100,
                                outcome,
                            },
                            sig: Sig::default(),
                        },
                        ms_ts: [(level as u8).wrapping_mul(2) + 1, 0, 0, 0, 0, 0],
                        acc: maker_acc,
                    },
                    sig: Sig::default(),
                },
                timestamplet: Timestamplet {
                    timestamplet_time: (level + 1) as u32,
                    timestamplet_outcome: outcome,
                    timestamplet_val: 101,
                },
            },
            server_sig: Sig::default(),
        };

        // The next (outer) game reinvests this node's full claim.
        available = available
            .checked_add(maker_stake)
            .expect("no overflow in test");
        source = OnrampingSource::AggregatedResult(AggregatedResultArgs {
            from: Box::new(AggregateEnum::Args(Box::new(aggregate))),
            amt_to_take: available,
        });
    }

    // The outermost AggregateEnum that wraps everything.
    let outermost: AggregateEnum = match source {
        OnrampingSource::AggregatedResult(AggregatedResultArgs { from, .. }) => *from,
        _ => unreachable!("depth >= 1 builds at least one nested aggregate"),
    };
    (outermost, available)
}

#[test]
fn test_recursive_input_10_depth_is_accepted() {
    const DEPTH: usize = 10;
    flush_guard_fresh(|| {
        let taker = [0xAA; 20];
        let makers: Vec<[u8; 20]> = (0..DEPTH).map(|i| [i as u8; 20]).collect();
        let initial_amt: u64 = 100;
        let maker_stake: u64 = 100;

        // Fund the ledger: taker pot + each maker's stake.
        storage::onramped_bal::set(&Asset::USDC.into(), &taker.into(), &U::from(initial_amt));
        for acc in &makers {
            storage::onramped_bal::set(&Asset::USDC.into(), &acc.into(), &U::from(maker_stake));
        }

        let (top, expected_claim) =
            build_recursive_aggregate(DEPTH, initial_amt, maker_stake, taker, &makers);

        // Expected claim after 10 consecutive wins: 100 + 10*100.
        assert_eq!(expected_claim, 100 + (DEPTH as u64) * maker_stake);

        let AggregateEnum::Args(top_agg) = &top else {
            panic!("the outermost node must be an Args aggregate");
        };
        let signable = SignableAggregateArgs::from(&top_agg.args);
        let (claim, returned_taker, returned_asset) = unsafe {
            recurse_aggregate_args(&Imm::default(), &top_agg.args, &signable, &Sig::default())
                .expect("10-deep recursive blob must be accepted")
        };

        assert_eq!(claim, expected_claim, "taker claim mismatch");
        assert_eq!(returned_taker, taker, "taker address mismatch");
        assert_eq!(returned_asset, Asset::USDC.addr(), "asset mismatch");

        // Report the serialized calldata size for on-chain postability.
        let blob = borsh::to_vec(&Args::AggregateToOfframp {
            args: top.clone(),
            is_taker: true,
        })
        .expect("borsh serialization");
        eprintln!(
            "recursive input depth {}: {} aggregate nodes, borsh calldata = {} bytes",
            DEPTH,
            DEPTH,
            blob.len()
        );
    });
}

// Report how the serialized calldata grows with recursion depth, so we can
// pick the maximum postable depth. Building + serializing needs no storage.
#[test]
fn report_calldata_growth_by_depth() {
    let taker = [0xAA; 20];
    for depth in [1usize, 5, 10, 20, 50, 100] {
        let makers: Vec<[u8; 20]> = (0..depth).map(|i| [i as u8; 20]).collect();
        let (top, _) = build_recursive_aggregate(depth, 100, 100, taker, &makers);
        let blob = borsh::to_vec(&Args::AggregateToOfframp {
            args: top,
            is_taker: true,
        })
        .unwrap();
        eprintln!("depth {depth:>3}: borsh calldata = {} bytes", blob.len());
    }
}
