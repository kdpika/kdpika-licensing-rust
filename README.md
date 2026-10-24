# KDPika Licensing — official Rust SDK

Add license activation, verified access and feature limits to native desktop apps,
CLI tools and trusted Rust web backends. KDPika is language-independent: Rust uses
the same API v1 and signed license contract as Python, PHP, JavaScript and C#.

Version **0.1.0**, MIT licensed. Rust **1.90+**, edition 2021.
Official registry package: [kdpika-licensing on crates.io](https://crates.io/crates/kdpika-licensing).

Official source: [kdpika/kdpika-licensing-rust](https://github.com/kdpika/kdpika-licensing-rust).
The separate [Python SDK repository](https://github.com/kdpika/kdpika-licensing-python)
is for Python only; Rust does not depend on it.

## Install from crates.io

```sh
cargo add kdpika-licensing@0.1.0
```

Or add it to Cargo.toml:

```toml
[dependencies]
kdpika-licensing = "0.1.0"
```

Import it as `kdpika_licensing`. Commit your application's Cargo.lock to retain
the resolved release/dependency versions. To enable the optional async worker:

```sh
cargo add kdpika-licensing@0.1.0 --features async
```

## Install from GitHub

To use the verified SDK code directly, add this pinned dependency to Cargo.toml:

```toml
[dependencies]
kdpika-licensing = { git = "https://github.com/kdpika/kdpika-licensing-rust", rev = "22d2032ea1ef06434878674b5baddaa93af769d1" }
```

Or use a local checkout if you prefer:

Clone this repository next to your application, using the directory name below:

```sh
git clone https://github.com/kdpika/kdpika-licensing-rust.git kdpika-rust-sdk
```

Add the path dependency shown below. Before distributing your application, pin a
reviewed commit and commit the application's Cargo.lock; do not rely on a moving
branch as a release identifier. This repository contains only the Rust client,
its tests and examples, not KDPika's proprietary SaaS backend.

## Install from the official source download

Download [kdpika-rust-sdk-0.1.0.zip](https://licensing.kdpika.com/docs/downloads/kdpika-rust-sdk-0.1.0.zip)
from the [Rust guide](https://licensing.kdpika.com/docs/v1/sdks/rust), compare the
SHA-256 shown there, and extract it next to your application. It contains a single
`kdpika-rust-sdk` directory. Add it to your application's `Cargo.toml`:

```toml
[dependencies]
kdpika-licensing = { path = "../kdpika-rust-sdk" }
```

Build your application with Cargo; commit its `Cargo.lock`. For the SDK's tests:

```sh
cd kdpika-rust-sdk
cargo test --locked
cargo test --locked --all-features
```

There is no runtime dependence on PyPI, GitHub, Font Awesome or an external
licensing service. Runtime HTTPS goes to your configured KDPika origin. Cargo
downloads the declared open-source dependencies during development/build.
No Rust installation is needed on KDPika's PHP hosting.

## Quickstart

1. Choose/create a product in your KDPika workspace and copy its **Public App ID**.
2. Create a **Sandbox license**. Use its matching buyer email when activating.
3. Choose a dedicated private, persistent state directory, not your application
   install folder or a public web directory. Keep it across launches/upgrades.
4. Add `pdf_export` to that product's plan if you want to test this example feature.

```rust
use kdpika_licensing::{Environment, ErrorCode, LicenseClient};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = LicenseClient::builder(std::env::var("KDP_APP_ID")?)
        .environment(Environment::Sandbox)
        .storage_dir(std::env::var("KDP_STATE_DIR")?)
        .application_name("Your Application")
        .application_version("1.0.0")
        .build()?;

    let mut result = client.check();
    if result.error_code() == Some(ErrorCode::NotActivated) {
        // In a desktop app, obtain these privately from your activation form.
        result = client.activate(
            &std::env::var("KDP_LICENSE_KEY")?,
            std::env::var("KDP_CUSTOMER_EMAIL").ok().as_deref(),
        );
    }
    if !result.valid() {
        eprintln!("{}", result.message());
        return Ok(());
    }
    if result.feature("pdf_export") {
        println!("PDF export is included.");
    }
    Ok(())
}
```

Never enable an app just because a key exists, `activated()` is true, a readable
payload says active, or an HTTP request returned 200. Require `result.valid()`
before protected work; feature helpers also fail closed. No developer management
API key belongs in a distributed app. Never put license keys, tokens or customer
email in logs, URLs, screenshots or command-line arguments.

## Lifecycle

| Method | Behavior |
| --- | --- |
| `LicenseClient::builder(app_id)` | Creates configuration; defaults to **Sandbox**, not Python's legacy Live default |
| `activate(key, email)` | Online activation of this installation; refuses blind replacement if already activated |
| `check()` | Verified cache until next check is due; otherwise online validation |
| `validate()` | Tries online; network loss alone may use authentic signed offline grace |
| `validate_online()` | Online response required; never falls back to offline cache |
| `deactivate()` | Releases the server seat, then clears credentials/cache and preserves identity |
| `activated()` | Returns `Result<bool, Error>` for token presence only, not permission |
| `reset_local()` | Local recovery only; does **not** release a server seat |
| `feature(name)` / `feature_value(name)` | Obtains a checked decision and gates the requested entitlement |

`deactivate()` returns an acknowledgement: check **`result.deactivated()`**. It
deliberately has `valid() == false`, no license signature and no features. This
is safer than treating successful seat release as permission to run the app.
Unlike some Python calls, an existing token cannot be silently reactivated;
validate it or deactivate first. No automatic mutation retries are made.

## Results and limits

Result fields are private: callers cannot construct or mutate an accepted result.
`valid()` and feature helpers re-evaluate the signed deadlines at use. Keep calling
`check()` at appropriate points; a held result is not permanent permission.

```rust
# use kdpika_licensing::LicenseClient;
# fn limits(client: &LicenseClient) {
let result = client.check();
if result.valid() && result.feature("pdf_export") {
    if let Some(limit) = result.limit("seat_count") {
        // Apply this nonnegative integer according to your app's own rules.
        println!("Permitted seats: {limit}");
    }
}
# }
```

`feature()` accepts boolean true or a positive numeric value, not the string
`"true"`. `feature_value()` returns a detached JSON value only while authorized.
`limit()` returns a `u64` only for a nonnegative integer; missing, negative,
fractional, boolean and out-of-range values return `None`, **not unlimited**.
Status, license ID, original expiry/grace, safe error code, source, signature
status, Retry-After and bounded request ID have read-only accessors. `Debug`
does not expose payloads, customer data, tokens, keys, identity or storage paths.

## Configuration

| Builder option | Default / rule |
| --- | --- |
| `environment(Environment::Sandbox/Live)` | Sandbox by default; choose Live explicitly for real customers |
| `base_url(origin)` | `https://licensing.kdpika.com`; HTTPS origin only, no path/query/fragment/credentials |
| `application_name(name)` | `Licensed Rust Application`; descriptive device label, not a fingerprint |
| `application_version(version)` | Empty; set your software version to enforce configured server version rules |
| `storage_dir(absolute_path)` | OS user application-data directory plus origin/product/environment namespace |
| `timeout(Duration)` | 15 seconds total per request, >0 and <=120; redirects and automatic retries disabled |
| `verification_key(VerificationKey)` | Optional independently trusted Ed25519 pin; cannot silently rotate |
| `transport(Arc<dyn Transport>)` | Advanced trusted byte-preserving transport adapter |
| `clock(fn)` | Advanced test-only injection; production uses UTC |
| `allow_loopback_http(true)` | Explicit local tests only; never permits remote plain HTTP |

Public-key retrieval uses only the configured HTTPS origin. Envelope-supplied
keys never establish trust. Optional pins validate algorithm, key ID derived
from the public key and canonical base64url. The parser rejects duplicate keys,
invalid Unicode/UTF-8, excess depth, oversized/nonfinite JSON and trailing data.
PHP numeric lexemes and Unicode escaping survive storage/reload; this contract
is **not** generic RFC 8785/JCS. `payload_json`, when supplied, must match the
authenticated payload. Binding includes App ID, environment and installation hash.

## Offline and storage security

- Fresh checks/cache carry signed `next_validation_at`, `offline_grace_until`
  and explicit nullable `expires_at`. Local settings cannot extend them.
- An explicit server refusal, HTTP 429/503/redirect, bad signature, invalid JSON
  or binding mismatch never unlocks an old cached grant. Expiry wins over grace.
- UTC rollback is checked against signed server time and a saved checkpoint
  (300-second tolerance). Held decisions also use monotonic elapsed time.
- Cache/credentials are atomic files under an OS-locked namespace. Stable random
  identity is reused across clients/processes; activation is serialized. Lock
  contention fails safely after a bounded wait rather than freezing indefinitely.
- macOS/Linux: owned 0700 directories, 0600 files, no symlink state files, no-follow
  opens and single-link checks. Use a **dedicated** state directory: permissions
  on that directory are tightened, so do not pass a shared Documents/home folder.
- Windows: atomic files/OS file locking are used, but POSIX modes do not set Windows
  ACLs. Use a private per-user LOCALAPPDATA directory and verify its inherited ACLs
  in your installer. This initial release does not configure a Windows DACL or an
  OS credential vault; files are not automatically encrypted.
- `zeroize` is used for owned session token cleanup and async queued credentials;
  it cannot guarantee every allocator/HTTP/OS copy is scrubbed. No absolute local
  anti-tampering claim is made. A device owner can patch code or restore snapshots.
  Enforce high-value online actions on an authenticated backend too.

## Async, web and desktop apps

For Tokio-based backends or native UI event loops:

```toml
kdpika-licensing = { path = "../kdpika-rust-sdk", features = ["async"] }
```

```rust
# use kdpika_licensing::{AsyncLicenseClient, LicenseClient};
# async fn example() -> Result<(), kdpika_licensing::Error> {
let client = AsyncLicenseClient::build(
    LicenseClient::builder("YOUR_PUBLIC_APP_ID")
).await?;
let result = client.check().await?;
if result.valid() && result.feature("pdf_export") {
    // Run the permitted feature.
}
# Ok(()) }
```

The adapter has a bounded queue and a dedicated worker; blocking networking and
its destructor stay outside Tokio. Dropping a future does not undo an already
started activation/deactivation. Reconcile its outcome before retrying.
Tauri/egui/iced apps should keep the client and token in the Rust/native side and
return narrow decisions to the UI. This crate is not a GUI toolkit or a browser
WASM client. Web apps still require their own login, tenant authorization and CSRF.

## Errors and recovery

Lifecycle operations return a safe `LicenseResult`; initialization and explicit
storage operations return `Result<_, Error>`. `ErrorCode::as_str()` provides
stable uppercase identifiers. Use `request_id()` as a safe support reference.

- `NOT_ACTIVATED`: ask for a license key and matching email privately.
- `ALREADY_ACTIVATED`: check or release the existing activation first.
- `INVALID_LICENSE`: check key/email/product/environment; no account data is exposed.
- Device/activation limits: release an old authorized installation or contact publisher.
- Expired/revoked/suspended/cancelled/not-started: no feature grant; show the safe message.
- `NETWORK_ERROR`: reconnect; do not blindly retry uncertain activation/deactivation.
- `RATE_LIMITED`: respect Retry-After; no busy retry loop.
- Signature/key rotation/time errors: never bypass checks; correct clock or contact publisher.
- Storage/busy errors: resolve private directory access/active lock; do not regenerate identity
  to circumvent installation or activation limits.

## Examples, tests and release boundaries

`examples/session.rs` provides activate/check/validate/online/deactivate; the async
example checks a license without blocking its runtime. Cross-language PHP vectors
and synthetic signing keys in `tests/` are public test data, never production keys.
The explicit ignored backend test requires a separate local PHP/MySQL fixture;
normal `cargo test` does not contact your platform or consume real license seats.

Rust 1.90 checks passed on Windows, Linux and macOS in the
[verified GitHub Actions run](https://github.com/kdpika/kdpika-licensing-rust/actions/runs/40098311126)
for commit `22d2032ea1ef06434878674b5baddaa93af769d1`: formatting, Clippy,
unit/lifecycle/doc tests and Cargo package verification. The local real PHP/MySQL
acceptance ran on Apple Silicon macOS; CI uses synthetic local fixtures, not your
production account. This does not certify a packaged GUI, installer ACLs or a
native credential vault. Verify those on your distribution target.
The SDK source is public on GitHub. Native-vault/UI parity is not claimed.
The platform/backend remains proprietary; this client SDK alone is MIT licensed.
The website ZIP is an immutable earlier source snapshot with its separately
published checksum; registry packages and Git commits have their own identities.

Support: [hi@kdpika.com](mailto:hi@kdpika.com) ·
[All languages](https://licensing.kdpika.com/docs/v1/sdks/overview) ·
[API v1](https://licensing.kdpika.com/docs/v1/api)
