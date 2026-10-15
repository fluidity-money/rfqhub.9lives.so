// SPDX-License-Identifier: MIT
pragma solidity 0.8.30;

contract UpgradeableProxy {
    event Upgraded(address indexed implementation);
    event AdminChanged(address previousAdmin, address newAdmin);

    bytes32 constant SLOT_IMPL = bytes32(uint256(keccak256('eip1967.proxy.implementation')) - 1);
    bytes32 constant SLOT_ADMIN = bytes32(uint256(keccak256('eip1967.proxy.admin')) - 1);

    /* Slots below are copied from ../src/storage.rs: */

    /// @dev wasCreated this contract?
    bool private wasCreated;
    /// @dev ed25519Count of the amount of public keys we use.
    uint256 private ed25519Count;
    /// @dev ed25519Slots that we use for public keys. The ed25519 public key.
    mapping(uint256 => bytes32) private ed25519Slot;
    /// @dev EOA owner of this account that we use. Will be the admin.
    address private ethereumOwner;

    mapping(uint256 => uint256) private timestamps;

    uint256 private version;

    constructor(
        address _impl,
        address _admin,
        bytes32 _publicKey
    ) {
        bytes32 slotImpl = SLOT_IMPL;
        bytes32 slotAdmin = SLOT_ADMIN;
        assembly {
            sstore(slotImpl, _impl)
            sstore(slotAdmin, _admin)
        }
        emit Upgraded(_impl);
        emit AdminChanged(address(0), _admin);
        wasCreated = true;
        ed25519Count = 1;
        // The accounts system has a bug where the operation for slot_map is
        // flipped. This code sets the correct field:
        bytes32 correctEdSlot = keccak256(abi.encode(uint256(2), uint256(0)));
        assembly {
            sstore(correctEdSlot, _publicKey)
        }
        ethereumOwner = _admin;
        version = 2;
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
            assembly {
                sstore(slotImpl, newImpl)
            }
            if (data.length > 0) {
                (bool rc,) = newImpl.delegatecall(data);
                require(rc, "delegatecall failed");
            }
        } else {
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
