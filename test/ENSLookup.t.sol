// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;

import {Test, console} from "forge-std/Test.sol";
import {ENSLookup} from "../contracts/ENSLookup.sol";

contract ENSLookupTest is Test {
    ENSLookup public ensLookup;

    function setUp() public {
        ensLookup = new ENSLookup();
    }

    function test_nameExists() public {
        string memory name = ensLookup.getNameForAddress(0xae2Fc483527B8EF99EB5D9B44875F005ba1FaE13);
        console.log(name);
    }

    function test_nameDoesNotExist() public {
        string memory name = ensLookup.getNameForAddress(address(0xbeef));
        console.log(name);
    }

    function test_nodeExists() public view {
        bytes32 node = 0x7a525fbebcdbdab9a87c86dfa21175a44bc0f4907cf8320179bd738370250e5e;
        string memory name = ensLookup.getNameForNode(node);
        console.log(name);
    }

    function test_nodeDoesNotExist() public view {
        bytes32 node = bytes32(0);
        string memory name = ensLookup.getNameForNode(node);
        console.log(name);
    }
}
