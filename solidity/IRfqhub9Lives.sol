// SPDX-License-Identifier: MIT
pragma solidity 0.8.20;

interface IRfqhub9Lives {
    event BalanceChanged(
        address indexed recipient,
        address indexed asset,
        uint64 indexed userBal,
        uint256 version
    );

    event GlobalBalanceChanged(
        address indexed recipient,
        address indexed asset,
        uint64 indexed globalBal,
        uint256 version
    );
}
