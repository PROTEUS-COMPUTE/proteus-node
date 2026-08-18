# PROTEUS chain node

![Rust](https://img.shields.io/badge/Rust-000000?style=for-the-badge&logo=rust&logoColor=white) &nbsp; ![Substrate](https://img.shields.io/badge/Substrate-282828?style=for-the-badge) &nbsp; ![$PRTS](https://img.shields.io/badge/token-%24PRTS-9FFF00?style=for-the-badge&labelColor=1a1a1a)

The node of **PROTEUS**, a sovereign Layer-1 where mining is useful GPU
inference rather than hashing. This repository is the chain itself: block
production, finality, staking and emission. It is a fork of subtensor v1.2.5,
the last single-token release before dTAO.

Miners run [proteus-miner](https://github.com/PROTEUS-COMPUTE/proteus-miner).
Holders run [proteus-wallet](https://github.com/PROTEUS-COMPUTE/proteus-wallet).
This is what everything else talks to.

## The network

```
chain          PROTEUS, id "proteus", ss58 42
token          $PRTS, 9 decimals
block time     12 s, Aura for authorship, GRANDPA for finality
public rpc     wss://rpc.proteus-agent.com
```

## Run your own node

You do not have to trust our endpoint, or ask us for anything. The chain is
public: point a node at it, sync from genesis, and verify the balances yourself.

```bash
cargo build --release
./target/release/node-subtensor \
  --chain ./proteus-mainnet-raw.json \
  --base-path /data \
  --name your-node-name \
  --bootnodes \
    /dns4/rpc.proteus-agent.com/tcp/30333/p2p/12D3KooWSnDNsH7ciW41mAJPvTbu2uChLeVKHrSAKJnu2cSvrpa8 \
    /dns4/rpc-tokyo.proteus-agent.com/tcp/30333/p2p/12D3KooWPQ2qzSkaavDPum5s98z9PztpPq8dSsiyi3qZ1HLUthWQ
```

Two bootnodes on two continents, so a node can still find the network when one
of them is unreachable. They are given by name rather than by address, so they
keep working if a machine ever moves. They are only an entry point: once connected, a node
discovers every other peer on its own.

The long `12D3KooW…` part is the peer identity, the public half of the node
key. It is what lets you verify you reached the right node rather than whoever
took that address.

The node syncs from genesis and needs nothing from us to do it: no API key, no
allowlist, no permission. Add `--rpc-external` to serve your own applications.

`proteus-mainnet-raw.json` is the genesis the live chain runs on. Its sha256 is
`81e67ac81159684bd84a2508e17ce8cfa7b70bd5ae82cf44156803fe863db362`, and it is
the same file, byte for byte, as the one inside the production containers. A
different spec is a different chain, so check it rather than trust it.

`proteus-mainnet-plain.json` is the same genesis in readable form, for anyone
who wants to inspect the initial balances.

## What PROTEUS changes from upstream

306 added lines across twelve files, listed in [NOTICE](NOTICE) together with
the command that prints them. In short: the token and
its properties, the genesis, and staking that can be locked for a chosen
duration by the runtime itself rather than by a custodian.

## Build

Requires the toolchain pinned in `rust-toolchain.toml` and a Linux host (WSL2
on Windows). The WASM target comes with it.

## License

The upstream subtensor code is public domain under [The Unlicense](LICENSE).
The changes PROTEUS made to it are MIT, see [LICENSE-PROTEUS](LICENSE-PROTEUS).
[NOTICE](NOTICE) says which files those are, and
[README-subtensor.md](README-subtensor.md) is the upstream readme kept as it was.
