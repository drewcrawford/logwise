# `logwise_v1` guest-to-host ABI

The optional wasm import is:

```wat
(import "logwise_v1" "emit" (func (param i32 i32) (result i32)))
```

The pointer and length borrow one complete envelope for the duration of the
call. The host must copy bytes it retains. Return values are `0` accepted, `1`
unavailable, `2` version mismatch, `3` dropped, and any other value a host
error. A build whose host does not provide the import leaves the `host-abi`
feature disabled; `Transport::emit` then returns `Unavailable` without adding a
required wasm import. Enabling the feature declares that the embedder supplies
the import at instantiation.

All integers are little-endian. Strings are a `u16` byte length followed by
UTF-8. Optional strings are a `u8` presence tag followed by a string.

| Field | Encoding |
|---|---|
| magic | `LW1\0` |
| ABI version | `u16`, currently 1 |
| flags | `u16`; bit 0 means this envelope truncated a value |
| complete envelope length | `u32` |
| sequence, dropped before, truncated before | three `u64`s |
| worker identity | `u64` |
| context ID and flags | two `u64`s |
| links | `u16` count, then ID/flags `u64` pairs |
| severity, class, kind | three `u8` facade discriminants |
| event, package, target, module | four strings |
| domain and test identity | two optional strings |
| location | presence tag, then file string, line `u32`, column `u32` |
| omitted fields | `u32` |
| fields | `u16` count, then entries below |
| message | presence tag, then a dynamic string |

Each field entry is its static name string, privacy `u8`, detail `u8`, value
tag, and payload. Value tags are: 1 bool (`u8`), 2 signed integer (two's-
complement `u64`), 3 unsigned integer (`u64`), 4 float (`f64::to_bits` as
`u64`), 5 string, 6 debug text, and 7 display text.

Secret fields are never encoded. Dynamic strings are truncated at a valid
UTF-8 boundary to the configured limit and counted; static schema strings that
cannot fit a `u16` fail the envelope instead. Every call is independently
framed and sequence-numbered, allowing a host to stream or mirror bytes as they
arrive rather than calling back into a possibly stalled guest.

[`golden/logwise_v1_event.hex`](golden/logwise_v1_event.hex) is the canonical
version-1 vector. The Rust test decodes it with an independent minimal-host
parser; host projects should consume the same file.
