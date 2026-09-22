// SPDX-License-Identifier: MIT
pragma solidity 0.8.20;

import {Test} from "forge-std/Test.sol";

import {IArbFoundry} from "./IArbFoundry.sol";

import "./RfqhubTypes.sol";

contract Erc20 {
    function transfer(address recipient, uint256 amt) external pure returns (bool) {
        return true;
    }

    function transferFrom(address from, address to, uint256 amt) external pure returns (bool) {
        return true;
    }
}

// arbos-foundry does not provide Superposition's SHA-512 precompile. The
// aggregate's signable hash still calls it even though signature validation is
// disabled in the arbos build, so return a deterministic 64-byte test digest.
contract Sha512Mock {
    fallback() external {
        assembly {
            mstore(0x00, 1)
            mstore(0x20, 2)
            return(0x00, 0x40)
        }
    }
}

contract TestRfqhub is Test {
    using RfqhubTypes for address;

    // Canonical USDC asset address, mirroring superposition-assets `Asset::USDC`.
    address internal constant USDC = 0xaf88d065e77c8cC2239327C5EDb3A432268e5831;

    // Superposition SHA-512 precompile used by SignableAggregateArgs hashing.
    address internal constant SHA512 = 0x1F4350205A556587fF3a1F2CB627613685dAcB73;

    // Timestamps used by the test: expiry/timestamplet = 1, byte-array nonce = zero.
    uint32 internal constant T1 = 1;
    bytes6 internal constant T0 = bytes6(hex"000000000000");

    // Namespace the arbos-featured build of the contract uses to resolve an
    // account's recipient/owner instead of calling accounts.superposition.so.
    bytes32 internal constant ARBOS_RECIPIENT_NS = keccak256("arbos.recipient");

    address c;

    function setUp() external {
        c = IArbFoundry(address(vm)).deployStylusCode(
            "rfqhub.9lives.so.wasm"
        );
        // Plant the dumb ERC20 at the canonical USDC address so the contract's
        // eip20 transfer/transferFrom host calls hit real Solidity code.
        vm.etch(USDC, type(Erc20).runtimeCode);
        vm.etch(SHA512, type(Sha512Mock).runtimeCode);
    }

    function test_contractVersion() public {
        Args memory args;
        args.kind = ArgsKind.Version;
        (,bytes memory rd) = RfqhubTypes.sendCalldata(vm, c, args);
        (uint256 version) = abi.decode(rd, (uint256));
        assert(version > 0);
    }

    // In the arbos build the contract resolves an account's owner/recipient out
    // of a derived storage slot rather than calling accounts.superposition.so:
    //   get_acc_owner(acc) = storage_load( slot_map(acc, keccak("arbos.recipient")) )
    //   slot_map(x, y)     = keccak256(x32 || y32)
    function _setRecipientHint(address acc, address owner) internal {
        bytes32 slot = keccak256(
            abi.encodePacked(uint256(uint160(acc)), ARBOS_RECIPIENT_NS)
        );
        vm.store(c, slot, bytes32(uint256(uint160(owner))));
    }

    // Seed the contract's onramped balance for `account` directly, mirroring
    // how the Rust unit test calls `storage::onramped_bal::set`.
    // `onramped_bal(asset, account)` derives its slot as
    //   slot_map(account, slot_map(asset, SLOT)), SLOT = 3 (storage counter),
    // and slot_map(x, y) = keccak256(x.0 || y.0) with 32-byte (right-aligned) U.
    function _setOnrampedBal(address account, uint256 amt) internal {
        // inner = slot_map(USDC_as_U, U(3))
        bytes32 inner = keccak256(
            abi.encodePacked(uint256(uint160(USDC)), uint256(3))
        );
        // slot  = slot_map(account_as_U, inner)
        bytes32 slot = keccak256(
            abi.encodePacked(uint256(uint160(account)), inner)
        );
        vm.store(c, slot, bytes32(amt));
    }

    // Seed storage::global_bal(USDC). It follows paused and version in storage.rs,
    // so its slot is slot_map(USDC, U(2)). AggregateToOfframp decrements this
    // ledger by the amount transferred to the winner.
    function _setGlobalBal(uint256 amt) internal {
        bytes32 slot = keccak256(
            abi.encodePacked(uint256(uint160(USDC)), uint256(2))
        );
        vm.store(c, slot, bytes32(amt));
    }

    // Read back an onramped USDC balance via the contract's `Balance` entry
    // (writes storage::onramped_bal::get as a word). Used to confirm the
    // vm.store-seeded balances are actually visible to the stylus runtime.
    function _onrampedBal(address addr) internal returns (uint256) {
        Args memory a;
        a.kind = ArgsKind.Balance;
        a.balance.asset = USDC;
        a.balance.addr = addr;
        (,bytes memory rd) = RfqhubTypes.sendCalldata(vm, c, a);
        return abi.decode(rd, (uint256));
    }

    // Translation of the Rust unit test `test_erik_alex`.
    //
    // Erik (the taker) expects Bitcoin to be above $100 by timestamp 1 and puts
    // 500 USDC on it. Alex (the maker) bets it won't and offers 200 USDC as the
    // compensation if Erik is right - the highest offer, so he's matched. The
    // timestamplet reports Bitcoin = 101 at timestamp 1, so Erik wins. 500 (his
    // stake) + 200 (Alex's compensation) = 700 USDC are paid out to him.
    // Run against the arbos build (SPN_DEPLOY_TARGET=arbos, chain id 31337).
    function test_erikAlex() public {
        // Mirror the Rust test exactly: `erik_addr = [1u8; 20]` is twenty bytes
        // of 0x01 (0x0101..01), NOT address(1). alex = [0u8; 20] = address(0).
        address erik = address(0x0101010101010101010101010101010101010101); // taker
        address alex = address(0);                                          // maker

        // The transaction is run as the test contract; make it Erik's "owner"
        // so entry_recurse_aggregate_enum's SenderNotTaker check passes.
        _setRecipientHint(erik, address(this));

        // Fund both parties' onramped balances directly (as the Rust test does).
        _setOnrampedBal(erik, 500);
        _setOnrampedBal(alex, 200);
        _setGlobalBal(700);

        // Confirm the stylus runtime can actually read the seeded balances
        // (i.e. vm.store lands in the slot the stylus storage resolves).
        assertEq(_onrampedBal(erik), 500, "erik onramped bal readable");
        assertEq(_onrampedBal(alex), 200, "alex onramped bal readable");

        // Build the aggregate args, mirroring the Rust bundle verbatim.
        AggregateArgs memory aggArgs;

        // --- taker: Erik ---
        aggArgs.maker.args.taker.args.src.kind = OnrampingSourceKind.OnchainPot;
        aggArgs.maker.args.taker.args.src.onchainPot.accAddr = erik;
        aggArgs.maker.args.taker.args.src.onchainPot.amt = 500;
        aggArgs.maker.args.taker.args.src.onchainPot.asset = Asset.USDC;
        aggArgs.maker.args.taker.args.expiry = T1;
        aggArgs.maker.args.taker.args.msTs = T0;
        aggArgs.maker.args.taker.args.isUp = true;
        aggArgs.maker.args.taker.args.priceTarget = 100;
        aggArgs.maker.args.taker.args.outcome = Outcome.BitcoinPrice;
        aggArgs.maker.args.taker.sig = new bytes(64);

        // --- maker: Alex ---
        aggArgs.maker.args.src.kind = OnrampingSourceKind.OnchainPot;
        aggArgs.maker.args.src.onchainPot.accAddr = alex;
        aggArgs.maker.args.src.onchainPot.amt = 200;
        aggArgs.maker.args.src.onchainPot.asset = Asset.USDC;
        aggArgs.maker.args.msTs = T0;
        aggArgs.maker.args.acc = alex;
        aggArgs.maker.sig = new bytes(64);

        // --- timestamplet: Bitcoin price 101 at expiry timestamp 1 ---
        aggArgs.timestamplet.timestampletTime = T1;
        aggArgs.timestamplet.timestampletOutcome = Outcome.BitcoinPrice;
        aggArgs.timestamplet.timestampletVal = 101;

        // The winning 700 USDC should be transferred to the taker's owner (the
        // test contract), mirroring the (700, erik, asset) the Rust test asserts.
        vm.expectCall(
            USDC,
            abi.encodeCall(Erc20.transfer, (address(this), uint256(700)))
        );

        Args memory args;
        args.kind = ArgsKind.AggregateToOfframp;
        args.aggregateToOfframp.args.kind = AggregateEnumKind.Args;
        args.aggregateToOfframp.args.args.args = aggArgs;
        args.aggregateToOfframp.args.args.serverSig = new bytes(64);
        args.aggregateToOfframp.isTaker = true;

        (bool ok,) = RfqhubTypes.sendCalldata(vm, c, args);
        assertTrue(ok, "aggregate to offramp");
    }

    function test_wtf() external {
        bytes memory cd = "0500003a804fe6f047f8a974701492025aa5adb61ab696640000000000000000003a804fe6f047f8a974701492025aa5adb61ab6966400000000000000003380b26a6ab27fd0000001640000000000000000f5b5a1766bc19d643d5d2f10db72edaba99230cd94c5bbe0f5e288787d54c4ad35b4609430aa88900f06295193db144b432a86b0abe21f0f8f57a1037c315d0c6ab27fd300003a804fe6f047f8a974701492025aa5adb61ab6964a1e004ca4b8fde1820b7169bf9b43927318f9c0fb991359cc4c6964efb687bb755c1c88a2499d4006ebd71c99c1ab18818cfebfaa791aa47fff327cb0ce470d3380b26a000800000000000000006a6903b21915a482390ab7f7767724736b1ffe159bdc2e2272f8bb20e190991c7bf1b98f17c85c52915a8eed9590d508dfbdb69b8b104f5b4b79817d8ce30a00";
        vm.createSelectFork("https://arbitrum-mainnet.infura.io/v3/87bd4c7ecce4449ba0746431da94685a");
        vm.etch(
            0x7193088e878F3b461e552b3D6c0c97F1051A7822,
                IArbFoundry(address(vm)).deployStylusCode(
                "rfqhub.9lives.so.wasm"
            ).code
        );
        vm.chainId(31337);
        (bool rc, bytes memory rd) = 0x7193088e878F3b461e552b3D6c0c97F1051A7822.call(cd);
        assert(rc);
        assertEq("", rd);
    }
}
