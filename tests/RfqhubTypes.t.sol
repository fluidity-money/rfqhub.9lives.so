// SPDX-License-Identifier: MIT
pragma solidity 0.8.20;

import {Test} from "../forge-lib/forge-std/src/Test.sol";

import "./RfqhubTypes.sol";

contract RfqhubTypesTest is Test {
    function test_u32TimestampsSerializeAsJsonNumbers() public pure {
        BundleTakerArgs memory taker;
        taker.src.kind = OnrampingSourceKind.OnchainPot;
        taker.expiry = 4_294_967_295;
        taker.outcome = Outcome.BitcoinPrice;

        assertEq(
            RfqhubTypes._bundleTakerArgs(taker),
            "{\"src\":{\"OnchainPot\":{\"acc_addr\":[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],\"amt\":0,\"asset\":\"USDC\"}},\"expiry\":4294967295,\"ms_ts\":[0,0,0,0,0,0],\"is_up\":false,\"price_target\":0,\"outcome\":\"BitcoinPrice\"}"
        );

        Timestamplet memory timestamplet;
        timestamplet.timestampletTime = 1;
        timestamplet.timestampletOutcome = Outcome.BitcoinPrice;

        assertEq(
            RfqhubTypes._timestamplet(timestamplet),
            "{\"timestamplet_time\":1,\"timestamplet_outcome\":\"BitcoinPrice\",\"timestamplet_val\":0}"
        );
    }

    function test_offrampJsonIncludesAssetTimestampAndAccountAddress() public pure {
        Args memory args;
        args.kind = ArgsKind.Offramp;
        args.offramp.args.asset = Asset.ARB;
        args.offramp.args.amt = 42;
        args.offramp.args.msTs = bytes6(hex"010203040506");
        args.offramp.args.accAddr = address(0x111122223333444455556666777788889999aAaa);
        args.offramp.serverSig = new bytes(64);

        assertEq(
            RfqhubTypes.toJson(args),
            string.concat(
                "{\"Offramp\":{\"args\":{\"asset\":\"ARB\",\"amt\":42,",
                "\"ms_ts\":[1,2,3,4,5,6],",
                "\"acc_addr\":[17,17,34,34,51,51,68,68,85,85,102,102,119,119,136,136,153,153,170,170]},",
                "\"server_sig\":\"0x",
                "0000000000000000000000000000000000000000000000000000000000000000",
                "0000000000000000000000000000000000000000000000000000000000000000\"}}"
            )
        );
    }
}
