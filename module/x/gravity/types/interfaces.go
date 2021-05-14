package types

import (
	sdk "github.com/cosmos/cosmos-sdk/types"
	"github.com/ethereum/go-ethereum/common"
	"github.com/ethereum/go-ethereum/common/hexutil"
	"github.com/gogo/protobuf/proto"
	tmbytes "github.com/tendermint/tendermint/libs/bytes"
)

// EthereumSignature represents one validtors signature for a given
// outgoing ethereum transaction
type EthereumSignature interface {
	proto.Message

	GetSigner() common.Address
	GetSignature() hexutil.Bytes
	GetStoreIndex(sdk.ValAddress) []byte
	Validate() error
}

// EthereumEvent represents a event from the gravity contract
// on the counterparty ethereum chain
type EthereumEvent interface {
	proto.Message

	GetNonce() uint64
	GetEthereumHeight() uint64
	Hash() tmbytes.HexBytes
	Validate() error
}

type OutgoingTx interface {
	// NOTE: currently the function signatures here don't match, figure out how to do this proprly
	// maybe add an interface arg here and typecheck in each implementation?

	// The only one that will be problematic is BatchTx which needs to pull all the constituent
	// transactions before calculating the checkpoint
	GetCheckpoint([]byte) []byte
	GetStoreIndex() []byte
}