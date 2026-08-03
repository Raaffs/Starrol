// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

/// @notice Standard interface for the deployed RISC Zero verifier contract
interface IRiscZeroVerifier {
    /**
     * @notice Verifies a RISC Zero Groth16 proof (seal) against an image ID and journal digest.
     * @dev Reverts if the proof is invalid or if the journal digest/image ID do not match.
     * @param seal The Groth16 proof provided by the RISC Zero prover
     * @param imageId The unique 32-byte identifier of the guest circuit code
     * @param journalDigest The SHA256 digest of the committed guest journal
     */
    function verify(
        bytes calldata seal,
        bytes32 imageId,
        bytes32 journalDigest
    ) external view;
}

contract ValidiumRoots {
    address public owner;
    IRiscZeroVerifier public verifier;

    bytes32 public appendImageId;
    bytes32 public updateImageId;

    bytes32[] public globalRoots;

    event AppendImageIdUpdated(bytes32 indexed oldImageId, bytes32 indexed newImageId);
    event UpdateImageIdUpdated(bytes32 indexed oldImageId, bytes32 indexed newImageId);
    event RootAppended(uint256 indexed index, bytes32 newRoot);
    event RootUpdated(uint256 indexed index, bytes32 oldRoot, bytes32 newRoot);

    modifier onlyOwner() {
        require(msg.sender == owner, "Not authorized");
        _;
    }

    constructor(address _verifier, bytes32 _appendImageId, bytes32 _updateImageId) {
        owner = msg.sender;
        verifier = IRiscZeroVerifier(_verifier);
        appendImageId = _appendImageId;
        updateImageId = _updateImageId;
    }

    function setAppendImageId(bytes32 _newAppendImageId) public onlyOwner {
        emit AppendImageIdUpdated(appendImageId, _newAppendImageId);
        appendImageId = _newAppendImageId;
    }

    function setUpdateImageId(bytes32 _newUpdateImageId) public onlyOwner {
        emit UpdateImageIdUpdated(updateImageId, _newUpdateImageId);
        updateImageId = _newUpdateImageId;
    }

    function appendRoot(bytes memory journal, bytes memory seal) public {
        verifier.verify(seal, appendImageId, sha256(journal));

        bytes32 newRoot = abi.decode(journal, (bytes32));

        globalRoots.push(newRoot);
        emit RootAppended(globalRoots.length - 1, newRoot);
    }

    function updateRoot(uint256 index, bytes memory journal, bytes memory seal) public {
        verifier.verify(seal, updateImageId, sha256(journal));

        (bytes32 oldRoot, bytes32 newRoot) = abi.decode(journal, (bytes32, bytes32));

        require(globalRoots[index] == oldRoot, "State mismatch");

        globalRoots[index] = newRoot;
        emit RootUpdated(index, oldRoot, newRoot);
    }

    function getGlobalRoots() public view returns (bytes32[] memory) {
        return globalRoots;
    }
}