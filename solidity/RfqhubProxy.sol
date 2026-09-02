// SPDX-License-Identifier: MIT
pragma solidity 0.8.30;

contract RfqhubProxy {
    event Upgraded(address indexed implementation);
    event AdminChanged(address previousAdmin, address newAdmin);

    bytes32 constant SLOT_IMPL = bytes32(uint256(keccak256('eip1967.proxy.implementation')) - 1);
    bytes32 constant SLOT_ADMIN = bytes32(uint256(keccak256('eip1967.proxy.admin')) - 1);

    /* Slots below are copied from superposition_librfq/src/storage.rs */

    bool paused;

    uint256 version;

    constructor(
        address _impl,
        address _admin
    ) {
        bytes32 slotImpl = SLOT_IMPL;
        bytes32 slotAdmin = SLOT_ADMIN;
        assembly {
            sstore(slotImpl, _impl)
            sstore(slotAdmin, _admin)
        }
        emit Upgraded(_impl);
        emit AdminChanged(address(0), _admin);
        version = 1;
    }

    fallback() external payable {
        bytes32 slotImpl = SLOT_IMPL;
        bytes32 adminImpl = SLOT_ADMIN;
        address admin;
        assembly {
            admin := sload(adminImpl)
        }
        if (msg.sender == admin) {
            if (msg.sig != Upgrade.upgradeToAndCall.selector) {
                revert("not calling upgrade");
            }
            (address newImpl, bytes memory data) = abi.decode(msg.data[4:], (address, bytes));
            emit Upgraded(newImpl);
            uint256 rdlen;
            assembly {
                sstore(slotImpl, newImpl)
                rdlen := returndatasize()
            }
            if (rdlen > 0) {
                (bool rc,) = newImpl.delegatecall(data);
                require(rc, "delegatecall failed");
            }
        } else {
            require(!paused, "contract paused");
            assembly {
                let impl := sload(slotImpl)
                calldatacopy(0, 0, calldatasize())
                let result := delegatecall(gas(), impl, 0, calldatasize(), 0, 0)
                returndatacopy(0, 0, returndatasize())
                switch result
                case 0 { revert(0, returndatasize()) }
                default { return(0, returndatasize()) }
            }
        }
    }
}

interface Upgrade {
    function upgradeToAndCall(address newImpl, bytes memory data) external;
}
