// SPDX-License-Identifier: MIT
pragma solidity 0.8.20;

// Glm5-2 generated code

import {Vm} from "../forge-lib/forge-std/src/Vm.sol";

enum Asset {
    USDC,
    ARB,
    WETH
}

enum Outcome {
    BitcoinPrice
}

struct FillerArgs {
    uint64 fusdcOnWin;
    address accSender;
}

struct OnchainPotArgs {
    address accAddr;
    uint64 amt;
    Asset asset;
}

enum OnrampingSourceKind {
    OnchainPot,
    AggregatedResult,
    OnchainPotAggregatedResult
}

struct OnrampingSource {
    OnrampingSourceKind kind;
    OnchainPotArgs onchainPot;
    bytes aggregatedResult;
}

struct BundleTakerArgs {
    OnrampingSource src;
    bytes6 expiry;
    bytes6 msTs;
    bool isUp;
    uint64 priceTarget;
    Outcome outcome;
}

struct BundleTaker {
    BundleTakerArgs args;
    bytes sig;
}

struct BundleMakerArgs {
    uint64 amt;
    BundleTaker taker;
    bytes6 msTs;
    address acc;
}

struct BundleMaker {
    BundleMakerArgs args;
    bytes sig;
}

struct OfframpArgs {
    Asset asset;
    uint64 amt;
    bytes6 msTs;
    address accAddr;
}

struct Timestamplet {
    bytes6 timestampletTime;
    Outcome timestampletOutcome;
    uint64 timestampletVal;
}

struct AggregateArgs {
    BundleMaker maker;
    Timestamplet timestamplet;
}

struct Aggregate {
    AggregateArgs args;
    bytes serverSig;
}

enum AggregateEnumKind {
    Args,
    Hash
}

struct AggregateEnum {
    AggregateEnumKind kind;
    Aggregate args;
    uint256 hash;
}

enum ArgsKind {
    Version,
    Source,
    Balance,
    OnrampingFromBal,
    Offramp,
    AggregateToOfframp
}

struct BalanceArgs {
    address asset;
    address addr;
}

struct OnrampingFromBalArgs {
    Asset asset;
    uint64 amt;
}

struct OfframpPayload {
    OfframpArgs args;
    bytes serverSig;
}

struct AggregateToOfframpArgs {
    AggregateEnum args;
    bool isTaker;
}

struct Args {
    ArgsKind kind;
    BalanceArgs balance;
    OnrampingFromBalArgs onrampingFromBal;
    OfframpPayload offramp;
    AggregateToOfframpArgs aggregateToOfframp;
}

/// @notice Solidity mirror of the top-level `Args` Rust enum in
/// `superposition_librfq/src/lib.rs`, plus the payload types needed to build it
/// in tests.
///
/// Rust has boxed recursive enum members (`Box<AggregateEnum>` inside
/// `OnrampingSource`). Solidity cannot directly express an infinitely recursive
/// value type, so those boxed recursive members are represented as raw Borsh
/// bytes (`aggregatedResult`) to be decoded or forwarded by the test harness as
/// needed. Rust `Sig = [u8; 64]` is represented as `bytes` and should be
/// exactly 64 bytes long.
///
/// `toJson` produces JSON matching `serde_json::to_string` output so the CLI
/// (`cli/src/main.rs`) can `serde_json::from_str` it back into `Args`.
/// Key serde conventions:
///   - Unit enum variants → bare strings: `"Version"`
///   - Struct enum variants → `{"VariantName":{...fields...}}`
///   - Newtype enum variants → `{"VariantName":<inner>}`
///   - Tuple enum variants  → `{"VariantName":[<items>]}`
///   - `[u8; N]` without `const_hex` → JSON array of ints: `[0,0,...]`
///   - `Sig` ([u8; 64]) with `const_hex` → `"0x..."` hex string
///   - `U` ([u8; 32]) → JSON array of 32 ints
///   - `Asset`/`Outcome` → bare string variant name
library RfqhubTypes {

    // ──────────────────── primitive helpers ────────────────────

    function _u2s(uint256 v) internal pure returns (string memory) {
        if (v == 0) return "0";
        uint256 tmp = v;
        uint256 d;
        while (tmp != 0) { d++; tmp /= 10; }
        bytes memory buf = new bytes(d);
        while (v != 0) { d--; buf[d] = bytes1(uint8(48 + v % 10)); v /= 10; }
        return string(buf);
    }

    function _b2s(bool v) internal pure returns (string memory) {
        return v ? "true" : "false";
    }

    function _assetStr(Asset a) internal pure returns (string memory) {
        if (a == Asset.USDC) return "USDC";
        if (a == Asset.ARB) return "ARB";
        return "WETH";
    }

    function _outcomeStr(Outcome) internal pure returns (string memory) {
        return "BitcoinPrice";
    }

    /// `[u8; N]` → JSON array of decimal ints, matching serde's default.
    function _u8Arr(bytes memory data) internal pure returns (string memory) {
        if (data.length == 0) return "[]";
        string memory r = "[";
        for (uint256 i = 0; i < data.length; i++) {
            if (i > 0) r = string.concat(r, ",");
            r = string.concat(r, _u2s(uint256(uint8(data[i]))));
        }
        return string.concat(r, "]");
    }

    function _u8Arr20(address a) internal pure returns (string memory) {
        return _u8Arr(abi.encodePacked(a));
    }

    function _u8Arr6(bytes6 b) internal pure returns (string memory) {
        return _u8Arr(abi.encodePacked(b));
    }

    function _u8Arr32(uint256 v) internal pure returns (string memory) {
        return _u8Arr(abi.encodePacked(v));
    }

    /// `bytes` → `"0x..."` lowercase hex, matching `const_hex` serde.
    function _hex(bytes memory data) internal pure returns (string memory) {
        bytes memory tbl = "0123456789abcdef";
        bytes memory r = new bytes(2 + data.length * 2);
        r[0] = "0"; r[1] = "x";
        for (uint256 i = 0; i < data.length; i++) {
            uint8 b = uint8(data[i]);
            r[2 + i * 2] = tbl[b >> 4];
            r[2 + i * 2 + 1] = tbl[b & 0x0f];
        }
        return string(r);
    }

    /// ASCII hex string (from `vm.ffi` stdout) → raw bytes.
    /// Extracts the last non-empty line so stderr noise from `vm.ffi`
    /// (which concatenates stdout+stderr) doesn't corrupt the hex.
    function _hexDec(bytes memory h) internal pure returns (bytes memory) {
        // Find the last non-empty line start.
        // Walk backwards, skipping trailing whitespace, then find the
        // beginning of the last line (char after a newline, or start).
        uint256 end = h.length;
        // Strip trailing whitespace
        while (end > 0 && _isWs(h[end - 1])) end--;
        if (end == 0) return new bytes(0);

        // Find start of the last line
        uint256 start = end;
        while (start > 0 && h[start - 1] != bytes1("\n")) start--;

        // Optionally skip a 0x prefix
        uint256 s = start;
        if (end - start >= 2 && h[start] == bytes1("0") && h[start + 1] == bytes1("x")) s = start + 2;

        uint256 n = end - s;
        require(n % 2 == 0, "odd hex");
        bytes memory r = new bytes(n / 2);
        for (uint256 i = 0; i < n; i += 2) {
            r[i / 2] = bytes1(_hc(h[s + i]) * 16 + _hc(h[s + i + 1]));
        }
        return r;
    }

    function _isWs(bytes1 c) internal pure returns (bool) {
        return c == bytes1("\n") || c == bytes1("\r") || c == bytes1(" ") || c == bytes1("\t");
    }

    function _hc(bytes1 c) internal pure returns (uint8) {
        if (c >= bytes1("0") && c <= bytes1("9")) return uint8(c) - 48;
        if (c >= bytes1("a") && c <= bytes1("f")) return uint8(c) - 87;
        if (c >= bytes1("A") && c <= bytes1("F")) return uint8(c) - 55;
        revert("bad hex char");
    }

    // ──────────────────── nested type serializers ────────────────────

    function _onchainPotArgs(OnchainPotArgs memory a) internal pure returns (string memory) {
        return string.concat(
            "{\"acc_addr\":", _u8Arr20(a.accAddr),
            ",\"amt\":", _u2s(uint256(a.amt)),
            ",\"asset\":\"", _assetStr(a.asset), "\"}"
        );
    }

    function _offrampArgs(OfframpArgs memory a) internal pure returns (string memory) {
        return string.concat(
            "{\"asset\":\"", _assetStr(a.asset),
            "\",\"amt\":", _u2s(uint256(a.amt)),
            ",\"ms_ts\":", _u8Arr6(a.msTs),
            ",\"acc_addr\":", _u8Arr20(a.accAddr), "}"
        );
    }

    function _aggregateEnum(AggregateEnum memory a) internal pure returns (string memory) {
        if (a.kind == AggregateEnumKind.Args) {
            return string.concat("{\"Args\":", _aggregate(a.args), "}");
        }
        return string.concat("{\"Hash\":", _u8Arr32(a.hash), "}");
    }

    function _aggregate(Aggregate memory a) internal pure returns (string memory) {
        return string.concat(
            "{\"args\":", _aggregateArgs(a.args),
            ",\"server_sig\":\"", _hex(a.serverSig), "\"}"
        );
    }

    function _aggregateArgs(AggregateArgs memory a) internal pure returns (string memory) {
        return string.concat(
            "{\"maker\":", _bundleMaker(a.maker),
            ",\"timestamplet\":", _timestamplet(a.timestamplet), "}"
        );
    }

    function _bundleMaker(BundleMaker memory a) internal pure returns (string memory) {
        return string.concat(
            "{\"args\":", _bundleMakerArgs(a.args),
            ",\"sig\":\"", _hex(a.sig), "\"}"
        );
    }

    function _bundleMakerArgs(BundleMakerArgs memory a) internal pure returns (string memory) {
        return string.concat(
            "{\"amt\":", _u2s(uint256(a.amt)),
            ",\"taker\":", _bundleTaker(a.taker),
            ",\"ms_ts\":", _u8Arr6(a.msTs),
            ",\"acc\":", _u8Arr20(a.acc), "}"
        );
    }

    function _bundleTaker(BundleTaker memory a) internal pure returns (string memory) {
        return string.concat(
            "{\"args\":", _bundleTakerArgs(a.args),
            ",\"sig\":\"", _hex(a.sig), "\"}"
        );
    }

    function _bundleTakerArgs(BundleTakerArgs memory a) internal pure returns (string memory) {
        return string.concat(
            "{\"src\":", _onrampingSource(a.src),
            ",\"expiry\":", _u8Arr6(a.expiry),
            ",\"ms_ts\":", _u8Arr6(a.msTs),
            ",\"is_up\":", _b2s(a.isUp),
            ",\"price_target\":", _u2s(uint256(a.priceTarget)),
            ",\"outcome\":\"", _outcomeStr(a.outcome), "\"}"
        );
    }

    function _onrampingSource(OnrampingSource memory s) internal pure returns (string memory) {
        if (s.kind == OnrampingSourceKind.OnchainPot) {
            return string.concat("{\"OnchainPot\":", _onchainPotArgs(s.onchainPot), "}");
        }
        // AggregatedResult / OnchainPotAggregatedResult: the boxed recursive
        // AggregateEnum is opaque bytes here. Serialize as hex — the test
        // harness must supply structured data if full serde fidelity is needed.
        if (s.kind == OnrampingSourceKind.AggregatedResult) {
            return string.concat("{\"AggregatedResult\":\"", _hex(s.aggregatedResult), "\"}");
        }
        return string.concat(
            "{\"OnchainPotAggregatedResult\":[",
            _onchainPotArgs(s.onchainPot), ",\"",
            _hex(s.aggregatedResult), "\"]}"
        );
    }

    function _timestamplet(Timestamplet memory t) internal pure returns (string memory) {
        return string.concat(
            "{\"timestamplet_time\":", _u8Arr6(t.timestampletTime),
            ",\"timestamplet_outcome\":\"", _outcomeStr(t.timestampletOutcome),
            "\",\"timestamplet_val\":", _u2s(uint256(t.timestampletVal)), "}"
        );
    }

    // ──────────────────── top-level toJson ────────────────────

    function toJson(Args memory a) internal pure returns (string memory) {
        if (a.kind == ArgsKind.Version) return "\"Version\"";
        if (a.kind == ArgsKind.Source) return "\"Source\"";
        if (a.kind == ArgsKind.Balance) {
            return string.concat(
                "{\"Balance\":{\"asset\":", _u8Arr20(a.balance.asset),
                ",\"addr\":", _u8Arr20(a.balance.addr), "}}"
            );
        }
        if (a.kind == ArgsKind.OnrampingFromBal) {
            return string.concat(
                "{\"OnrampingFromBal\":{\"asset\":\"", _assetStr(a.onrampingFromBal.asset),
                "\",\"amt\":", _u2s(uint256(a.onrampingFromBal.amt)), "}}"
            );
        }
        if (a.kind == ArgsKind.Offramp) {
            return string.concat(
                "{\"Offramp\":{\"args\":", _offrampArgs(a.offramp.args),
                ",\"server_sig\":\"", _hex(a.offramp.serverSig), "\"}}"
            );
        }
        // AggregateToOfframp
        return string.concat(
            "{\"AggregateToOfframp\":{\"args\":", _aggregateEnum(a.aggregateToOfframp.args),
            ",\"is_taker\":", _b2s(a.aggregateToOfframp.isTaker), "}}"
        );
    }

    // ──────────────────── sendCalldata ────────────────────

    /// @notice Serializes `args` to serde JSON, invokes `rfqhub-cli` via FFI
    ///         to get borsh-encoded hex, decodes it, and low-level calls `target`.
    /// @return success  Whether the call reverted.
    /// @return result   Return data (or revert reason on failure).
    function sendCalldata(Vm vm, address target, Args memory args)
        internal
        returns (bool success, bytes memory result)
    {
        string[] memory ffi = new string[](2);
        ffi[0] = "./rfqhub-cli";
        ffi[1] = toJson(args);
        // Foundry's vm.ffi auto-detects hex output and decodes it, so the
        // returned bytes are already the raw borsh payload — no further
        // decoding needed.
        bytes memory data = vm.ffi(ffi);
        (success, result) = target.call(data);
    }
}
