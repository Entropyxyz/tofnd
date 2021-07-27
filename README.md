# Tofnd: A gRPC threshold signature scheme daemon

Tofnd is a [gRPC](https://grpc.io/) server written in Rust that wraps the [tofn](https://github.com/axelarnetwork/tofn) threshold cryptography library.

# Setup

The gRPC protobuf file is a separate [submodule](https://github.com/axelarnetwork/grpc-protobuf/). To fetch it, please be sure that the `--recursive` flag is enabled:
```
git clone git@github.com:axelarnetwork/tofnd.git --recursive
```

`tofnd` uses the [hyperium/tonic](https://github.com/hyperium/tonic) Rust gRPC implementation, which requires:
* Rust `1.39` or greater for the `async_await` feature
    ```
    $ rustup update
    ```
* `rustfmt` to tidy up the code it generates
    ```
    $ rustup component add rustfmt
    ```

# Running the server

```
$ cargo run
```

Terminate the server with `ctrl+C`.

## Command line arguments

We use [clap](https://clap.rs/) to manage command line arguments.

Users can specify:
1. The port number of the gRPC server (default is 50051).
2. `mnemonic` operations for their `tofnd` instance (default is `Create`).
For more information, see on mnemonic options, see [Mnemonic](#mnemonic).

```
$ cargo run -- -h

USAGE:
    tofnd [OPTIONS]

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
    -m, --mnemonic <mnemonic>     [default: create]  [possible values: stored, create, import, update, export]
    -p, --port <port>             [default: 50051]
```

# Mnemonic

`Tofnd` uses the [tiny-bip39](https://docs.rs/crate/tiny-bip39/0.8.0) crate to enable users manage mnemonic passphrases. Currently, each party can use only one passphrase.

Mnemonic is used to enable _recovery_ of shares in case of unexpected loss. See more about recovery under the [Recover](#Recover) section.

## Mnemonic options

The command line API supports the following commands:

* `Noop` does nothing and always succeeds; useful when the container restarts with the same mnemonic.  

* `Create` creates a new mnemonic; Succeeds when there is no other mnemonic already imported, fails otherwise. The new passphrase is written in a file named _./export_.

* `Import` adds a new mnemonic from file _./import_ file; Succeeds when there is no other mnemonic already imported, fails otherwise.

* `Export` writes the existing mnemonic to file _./export_; Succeeds when there is an existing mnemonic, fails otherwise.

* `Update` updates existing mnemonic from file _./import_; Succeeds when there is an existing mnemonic, fails otherwise. The old passphrase is written to file _./export_.

If an _./export_ file already exists, then a new one is created with a new id, e.g. _./export_2_, _./export_3_, etc.

## Zeroization

We use the [zeroize](https://docs.rs/zeroize/1.1.1/zeroize/) crate to clear sensitive info for memory as a good procatie. The data we clean are related to the mnemonic:
1. entropy
2. passwords
3. passphrases

Note that, [tiny-bip39](https://docs.rs/crate/tiny-bip39/0.8.0) also uses `zeroize` internally.

# KV Store

To persist information between different gRPCs (i.e. _keygen_ and _sign_), we use a key-value storage based on [sled](https://sled.rs/).

`Tofnd` uses two separate KV Stores:
1. `Share KV Store`. Stores all user's shares when `keygen` protocol is completed, and uses them for `sign` protocol. Default path is _./kvstore/shares_.
2. `Mnemonic KV Store`. Stores the entropy of a mnemonic passphrase. This entropy is used to encrypt and decrypt users' sensitive info, i.e. the content of the `Share KV Store`. Default path is _./kvstore/mnemonic_.

## Security

**Important note**: Currently, the `mnemonic KV Store` is **not** encrypted. The mnemonic entropy is stored in clear text on disk. Our current security model assumes secure device access.

# Multiple shares

Multiple shares are handled internally. That is, if a party has 3 shares, the `tofnd` binary spawns 3 protocol execution threads, and each thread invokes `tofn` functions independently. At the end of the protocol, the outputs of all 3 party's shares are aggregated and a single result is created and sent to the client. Naturally, the same applies for any number of shares.

For `tofn` support on multiple shares, see [here](https://github.com/axelarnetwork/tofn#support-for-multiple-shares-per-party).

# gRPCs
Tofnd currently supports the following gRPCs:
1. `keygen`
2. `sign`
3. `recover`.

`Keygen` and `sign` use [bidirectional streaming](https://grpc.io/docs/what-is-grpc/core-concepts/#bidirectional-streaming-rpc) and `recover` is [unary](https://grpc.io/docs/what-is-grpc/core-concepts/#bidirectional-streaming-rpc).

## Diagrams

**TODO: change links after merge**

See a generic protocol sequence diagram, [here](https://github.com/axelarnetwork/tofnd/blob/readme/diagrams/protocol.pdf).

See [keygen](https://github.com/axelarnetwork/tofnd/blob/readme/diagrams/keygen.svg) and [sign](https://github.com/axelarnetwork/tofnd/blob/readme/diagrams/keygen.svg) diagrams of detailed message flow of each protocol. By opening the `.svg` files at a new tab (instead of previewing from github), hyperlinks will be available that will point you to the code block in which the underlying operations are implemented.

## Keygen
The _keygen_ gRPC executes the keygen protocol as implemented in [tofn](https://github.com/axelarnetwork/tofn) and described in [GG20](https://eprint.iacr.org/2020/540.pdf).

The initialization of _keygen_ is actualized by the following message:

```
message KeygenInit {
    string new_key_uid;  // keygen's identifier        
    repeated string party_uids;
    repeated uint32 party_share_counts;
    int32 my_party_index;       
    int32 threshold;
}
```

### Successful keygen
On success, the _keygen_ protocol returns a `SecretKeyShare` struct defined by `tofn`
```
pub struct SecretKeyShare {
    group: GroupPublicInfo,
    share: ShareSecretInfo,
}
```

This struct includes:
1. The information that is needed by the party in order to participate in subsequent _sign_ protocols that are associated with the completed _keygen_.
2. The `public key` of the current _keygen_.

Since multiple shares per party are supported, _keygen_'s result may produce multiple `SecretKeyShare`s. The collection of `SecretKeyShare`s is stored in the `Share KV Store` as the _value_ with the `key_uid` as _key_.

Each `SecretKeyShare` is then encrypted using the party's `mnemonic`, and the encrypted data is sent to the client as bytes, along with the `public key`. We send the encrypted `SecretKeyShare`s to facilitate _recovery_ in case of data loss.

The gRPC message of _keygen_'s data is the following:
```
message KeygenOutput {
    bytes pub_key = 1;                       // pub_key
    repeated bytes share_recovery_infos = 2; // recovery info
}
```

### Unsuccessful keygen

The `tofn` library supports fault detection. That is, if a party does not follow the protocol (e.g. by corrupting zero knowledge proofs, stalling messages etc), a fault detection mechanism is triggered, and the protocol ends prematurely with all honest parties composing a faulter list.

In this case, instead of the aforementioned result, _keygen_ returns a `Vec<Faulters>`, which is sent over the gRPC stream before closing the connection.

### File structure
_Keygen_ is implemented in [tofnd/src/gg20/keygen](https://github.com/axelarnetwork/tofnd/tree/recovery-api/src/gg20/keygen), which has the following file structure:

```
├── keygen
    ├── mod.rs
    ├── init.rs
    ├── execute.rs
    ├── result.rs
    └── types.rs
```

* In `mod.rs`, the handlers of protocol initialization, execution and aggregation of results are called. Also, in case of multiple shares, multiple execution threads are spawned.
* In `init.rs`, the verification and sanitization of the `Keygen Init` message is handled.
* In `execute.rs`, the instantiation and execution of the protocol is actualized.
* In `result.rs`, the results of all party shares are aggregated, validated and sent to the gRPC client.
* In `types.rs`, useful structs that are needed in the rest of the modules are defined.

## Sign
The _sign_ gRPC executes the sign protocol as implemented in [tofn](https://github.com/axelarnetwork/tofn) and described in [GG20](https://eprint.iacr.org/2020/540.pdf).

The initialization of _sign_ is actualized by the following message:

```
message SignInit {
    string key_uid;     // keygen's identifier
    repeated string party_uids;
    bytes message_to_sign;
}
```

### Successful sign

On success, the _keygen_ protocol returns a `signature` which is a `Vec<u8>`.

Since multiple shares per party are supported, _sign_'s result may produce multiple `signatures`s which are the same across all shares. Only one copy of the `signature` is sent to the gRPC client.

### Unsuccessful sign

Similarly to _keygen_, if faulty parties are detected during the execution of _sign_, the protocol is stopped and a `Vec<Faulters>` is returned to the client.

### Trigger recovery

_Sign_ is started with the special gRPC message `SignInit`.
```
message SignInit {
    string key_uid = 1;
    repeated string party_uids = 2;
    bytes message_to_sign = 3;
}
```

`key_uid` indicates the session identifier of an executed _keygen_. In order to be able to participate to _sign_, parties need to have their `share` info stored at the `Share KV Store` as _value_, under the _key_ `key_uid`. If this data is not present at the machine of a party (i.e. no `key_uid` exists in `Share KV Store`), a `need_recover` gRPC message is sent to the client and the connection is then closed. In the `need_recover` message, the missing `key_uid` is included.

```
message NeedRecover {
    string session_id = 1;
}
```

The client then proceeds by triggering _recover_ gRPC, and then starts the _sign_ again for the recovered party. Other participants are not affected.

### File structure
The keygen protocol is implemented in [tofnd/src/gg20/sign](https://github.com/axelarnetwork/tofnd/tree/recovery-api/src/gg20/sign), which, similar to _keygen_, has the following file structure:

```
├── sign
    ├── mod.rs
    ├── init.rs
    ├── execute.rs
    ├── result.rs
    └── types.rs
```

* In `mod.rs`, the handlers of protocol initialization, execution and aggregation of results are called. Also, in case of multiple shares, multiple execution threads are spawned.
* In `init.rs`, the verification and sanitization of `Sign Init` message is handled. If the absence of shares is discovered, the client sends a `need_recover` and stops.
* In `execute.rs`, the instantiation and execution of the protocol is actualized.
* In `result.rs`, the results of all party shares are aggregated, validated and sent to the gRPC client.
* In `types.rs`, useful structs that are needed in the rest of the modules are defined.

## Recover

As discussed in [keygen](#keygen) and [sign](#sign) section, the recovery of lost keys and shares is supported. In case of sudden data loss, for example due to a hard disk crash, parties are able to recover their shares. This is possible because each party sends it's encrypted secret info to the client before storing it inside the `Share KV Store`.

When _keygen_ is completed, the party's information is encryped and sent to the client. When the absence of party's information is detected during _sign_, `Tofnd` sends the `need_recover` message, indicating that recovery must be triggered.

Recovery is a [unary](https://grpc.io/docs/what-is-grpc/core-concepts/#bidirectional-streaming-rpc) gRPC. The client re-sends the `KeygenInit` message and the encrypted recovery info. This allows `Tofnd` to reconstruct the `Share KV Store` by decrypting the recovery info using the party's `mnemonic`.

```
message RecoverRequest {
    KeygenInit keygen_init = 1;
    repeated bytes share_recovery_infos = 2;
}
```

If _recovery_ was successful, a `success` message is sent, other wise `Tofnd` sends a `fail` message.

```
message RecoverResponse {
    enum Response {
        success = 0;
        fail = 1;
    }
    Response response = 1;
}
```

# Testing

## Honest behaviours

Both unit tests and integration tests are provided:
```
$ cargo test
```

## Malicious behaviours

`Tofn` supports faulty behaviours to test fault detection. These behaviours are only supported under the `malicious` feature. See more for Rust features [here](https://doc.rust-lang.org/cargo/reference/features.html).

`Tofnd` incorporates the `malicious` feature. You can run malicious tests by:
```
$ cargo test --all-features
```


