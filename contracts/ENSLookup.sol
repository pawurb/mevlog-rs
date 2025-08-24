// SPDX-License-Identifier: MIT

pragma solidity ^0.8.13;

interface IReverseRegistrar {
    function node(address addr) external returns (bytes32);
}

interface IEnsRegistry {
    function resolver(bytes32 node) external view returns (address);
}

interface IEnsResolver {
    function name(bytes32 node) external view returns (string memory);
    function addr(bytes32 node) external view returns (address);
}

contract ENSLookup {
    IReverseRegistrar public reverseRegistrar = IReverseRegistrar(0x9062C0A6Dbd6108336BcBe4593a3D1cE05512069);
    IEnsRegistry public ensRegistry = IEnsRegistry(0x00000000000C2E074eC69A0dFb2997BA6C7d2e1e);

    function getNameForNode(bytes32 node) public view returns (string memory) {
        address resolver = ensRegistry.resolver(node);
        if (resolver == address(0)) {
            return "";
        }
        return IEnsResolver(resolver).name(node);
    }

    function getAddressForNode(bytes32 node) public view returns (address) {
        address resolver = ensRegistry.resolver(node);

        if (resolver == address(0)) {
            return address(0);
        }

        return IEnsResolver(resolver).addr(node);
    }
}
