// SPDX-License-Identifier: MIT
pragma solidity 0.8.20;

import {Test} from "forge-std/Test.sol";
import {IArbFoundry} from "./IArbFoundry.sol";
import "./RfqhubTypes.sol";

contract Erc20 {
    function transfer(address, uint256) external pure returns (bool) { return true; }
    function transferFrom(address, address, uint256) external pure returns (bool) { return true; }
}
contract Sha512Mock {
    fallback() external {
        assembly {
            mstore(0x00, 1)
            mstore(0x20, 2)
            return(0x00, 0x40)
        }
    }
}

contract Probe is Test {
    using RfqhubTypes for address;

    address internal constant USDC = 0xaf88d065e77c8cC2239327C5EDb3A432268e5831;
    address internal constant SHA512 = 0x1F4350205A556587fF3a1F2CB627613685dAcB73;
    bytes6 internal constant T1 = bytes6(hex"000000000001");
    bytes6 internal constant T0 = bytes6(hex"000000000000");
    bytes32 internal constant ARBOS_RECIPIENT_NS = keccak256("arbos.recipient");

    address c;
    address erik = address(0x0101010101010101010101010101010101010101);
    address alex = address(0);

    function setUp() external {
        c = IArbFoundry(address(vm)).deployStylusCode("rfqhub.9lives.so.wasm");
        vm.etch(USDC, type(Erc20).runtimeCode);
        vm.etch(SHA512, type(Sha512Mock).runtimeCode);
    }

    function _setRecipientHint(address acc, address owner) internal {
        bytes32 slot = keccak256(abi.encodePacked(uint256(uint160(acc)), ARBOS_RECIPIENT_NS));
        vm.store(c, slot, bytes32(uint256(uint160(owner))));
    }

    function _setOnrampedBal(address account, uint256 amt) internal {
        bytes32 inner = keccak256(abi.encodePacked(uint256(uint160(USDC)), uint256(2)));
        bytes32 slot = keccak256(abi.encodePacked(uint256(uint160(account)), inner));
        vm.store(c, slot, bytes32(amt));
    }

    function _tryAggregate(address hintOwner) internal returns (bool ok) {
        _setRecipientHint(erik, hintOwner);
        _setOnrampedBal(erik, 500);
        _setOnrampedBal(alex, 200);

        AggregateArgs memory aggArgs;
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
        aggArgs.maker.args.msTs = T0;
        aggArgs.maker.args.acc = alex;
        aggArgs.maker.args.amt = 200;
        aggArgs.maker.sig = new bytes(64);
        aggArgs.timestamplet.timestampletTime = T1;
        aggArgs.timestamplet.timestampletOutcome = Outcome.BitcoinPrice;
        aggArgs.timestamplet.timestampletVal = 101;

        Args memory args;
        args.kind = ArgsKind.AggregateToOfframp;
        args.aggregateToOfframp.args.kind = AggregateEnumKind.Args;
        args.aggregateToOfframp.args.args.args = aggArgs;
        args.aggregateToOfframp.args.args.serverSig = new bytes(64);
        args.aggregateToOfframp.isTaker = true;

        (ok,) = RfqhubTypes.sendCalldata(vm, c, args);
        return ok;
    }

    function test_probe_msgSender() public {
        emit log_named_address("address_this", address(this));
        emit log_named_address("contract_c", c);
        emit log_named_uint("hint_address_this", _tryAggregate(address(this)) ? 1 : 0);
        emit log_named_uint("hint_addr0", _tryAggregate(address(0)) ? 1 : 0);
        emit log_named_uint("hint_c", _tryAggregate(c) ? 1 : 0);
        emit log_named_uint("hint_erik", _tryAggregate(erik) ? 1 : 0);
    }
}