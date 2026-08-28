// SPDX-License-Identifier: MIT
pragma solidity 0.8.20;

import {Test} from "forge-std/Test.sol";

import {IArbFoundry} from "./IArbFoundry.sol";

import {RfqhubTypes, ArgsKind, Args} from "./RfqhubTypes.sol";

contract Erc20 {
    function transfer(address recipient, uint256 amt) external pure returns (bool) {
        return true;
    }

    function transferFrom(address from, address to, uint256 amt) external pure returns (bool) {
        return true;
    }
}

contract Rfqhub9LIVES is Test {
    using RfqhubTypes for address;

    address c;

    function setUp() external {
        c = IArbFoundry(address(vm)).deployStylusCode(
            "rfqhub.9lives.so.wasm"
        );
    }

    function test_contractVersion() public {
        Args memory args;
        args.kind = ArgsKind.Version;
        (,bytes memory rd) = RfqhubTypes.sendCalldata(vm, c, args);
        (uint256 version) = abi.decode(rd, (uint256));
        assert(version > 0);
    }
}
