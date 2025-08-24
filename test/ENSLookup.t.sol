// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;

import {Test, console} from "forge-std/Test.sol";
import {ENSLookup} from "../contracts/ENSLookup.sol";

contract ENSLookupTest is Test {
    ENSLookup public ensLookup;

    function setUp() public {
        ensLookup = new ENSLookup();
    }

    function test_nodeExists() public view {
        bytes32 node = 0x7a525fbebcdbdab9a87c86dfa21175a44bc0f4907cf8320179bd738370250e5e;
        string memory name = ensLookup.getNameForNode(node);
        assertEq(name, "jaredfromsubway.eth");
    }

    function test_nodeDoesNotExist() public view {
        bytes32 node = bytes32(0);
        string memory name = ensLookup.getNameForNode(node);
        assertEq(name, "");
    }

    function test_addressExists() public view {
        address addr = ensLookup.getAddressForNode(0x58cd05464890eea8983a9e3667d4dde88c353b2922c1dbccf9f43622bc208b67);
        assertEq(addr, 0xae2Fc483527B8EF99EB5D9B44875F005ba1FaE13);
    }

    function test_addressDoesNotExist() public view {
        address addr = ensLookup.getAddressForNode(bytes32(0));
        assertEq(addr, address(0));
    }
}
