// SPDX-License-Identifier: UNLICENSED
pragma solidity 0.8.20;

import {Test} from "forge-std/Test.sol";

import {IArbFoundry} from "./IArbFoundry.sol";

import {IRfqhub9LIVES} from "../src/IRfqhub9LIVES.sol";

contract Rfqhub9LIVES is Test {
    IRfqhub9LIVES c;

    function setUp() external {
        c = IRfqhub9LIVES(IArbFoundry(address(vm)).deployStylusCode(
            "rfqhub_9lives.wasm"
        ));
    }

    function test_contractDeployed() public view {
        assertEq(123, c.hello());
    }
}
