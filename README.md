# KDPika Licensing Rust SDK

## Desktop / Web plan controls — 0.4.0

This source adds Business-only per-software-plan app access: `desktop`, `web`, or `both`. Starter and Pro automatically allow both under the same App ID and cannot create app-specific restrictions. It applies equally to subscriptions, one-time purchases and lifetime access. Devices/seats/credits belong to the same license; choosing both does not double its limits.

Configure the host application, not a customer-editable field:

```
LicenseClient::builder("YOUR_APP_ID").application_type("web").storage_dir("/private/web-license").build()?
```

Default: desktop. Configure the other app with the same App ID and key, its own context and private state folder. SDK storage namespaces include the context. All activation/validation/seat/credit requests send `X-Application-Type`, and the SDK checks signed `application_access` plus `application_type` on network and cached results. A scope-restricted license cannot activate without declaring a context. Errors: APPLICATION_NOT_ALLOWED, APPLICATION_TYPE_REQUIRED, APPLICATION_TYPE_MISMATCH, INVALID_APPLICATION_TYPE. Releasing a machine/seat remains possible after a restriction change.

Context is a declaration by trusted application code, not browser/hardware attestation. A user who controls executable code can modify it; valuable online operations need server-side authorization too. Offline clients learn changed policies only on reconnect and must obey the old signed deadline until then. Browser/WASM hardware fallback remains unsupported.

[Download the source ZIP](https://licensing.kdpika.com/docs/downloads/kdpika-rust-sdk-0.4.0.zip) · [Desktop / Web setup and migration](https://licensing.kdpika.com/docs/v1/licensing/application-access)

## Install this release

```sh
cargo add kdpika-licensing@0.4.0
```


Official SDK **0.4.0** for signed licensing, hardware binding, floating seats, metered credits and Desktop / Web access controls.

[Documentation](https://licensing.kdpika.com/docs/v1/sdks/rust) · [Download SDK ZIP](https://licensing.kdpika.com/docs/downloads/kdpika-rust-sdk-0.4.0.zip) · [GitHub source](https://github.com/kdpika/kdpika-licensing-rust)

## Supported licensing capabilities

| Capability | Behavior |
|---|---|
| Signed checks and offline access | Verify Ed25519 results; access never extends beyond signed expiry/offline policy. |
| Device matching | Required product-salted hardware hashes and X-Installation-ID; verify the signed device binding. Raw serials are not sent. |
| Floating licenses | Checkout, heartbeat and checkin with signed lease deadlines; handle ALL_SEATS_IN_USE. |
| Usage credits | Online-only consume with a persisted idempotency key; query balance and handle INSUFFICIENT_CREDITS. |
| Customer portal | Hosted self-service portal managed by the server, not an SDK payment or account feature. |

Advanced matching, floating seats, credits and portal availability depend on your workspace plan. Browser/WASM hardware fingerprint fallback is unsupported. Test on your target OS and in Sandbox before distributing your app.

## Install

```sh
cargo add kdpika-licensing@0.4.0
```


## Hardware matching, shared seats and usage credits

Requires the matching Phase B–D server. Native Windows/macOS/Linux collect
product-salted component hashes (never raw serials); activation and validation
send a required fingerprint and installation identity. Signed strict/fuzzy
hardware binding is checked for online/cache/offline results. Unavailable native
hardware fails closed (`HARDWARE_UNAVAILABLE`); VM detection is only a hint.
`ClientBuilder::fingerprint_provider` is a trusted native adapter/testing hook,
never an end-user-supplied identity. No browser/WASM fallback is implemented.

Floating licenses automatically checkout and start a stoppable heartbeat at the
signed interval. `checkout()` returns `Err(Error)` with `ErrorCode::AllSeatsInUse`;
`heartbeat()`, `checkin()` and `close()` return checked decisions. Signed lease,
private session-token hash and expiry gate cached/offline/retained feature access.
Explicit close is recommended; Drop attempts release, but abrupt process exits
cannot send a request and the bounded server lease must expire.

`consume(1, Some("durable_export_job_123"), None)` returns an immutable-by-ownership
`CreditConsumption` receipt. Catch `CreditError::InsufficientCredits { balance }`
or inspect `CreditError::code()`. `get_balance()` always fetches a signed online
balance. A replay returns the original transaction balance; the fresh balance can
be zero. A successful last-credit consumption still permits that operation.
Persist/reuse the same job key after an uncertain outcome: no automatic consume
retry. Deduplicate the actual job separately. Normal feature checks do not charge
credits; metered cache/offline checks deny `METERED_ONLINE_REQUIRED`. Async handles
expose the same operations. Cancelling an await does not undo an in-flight charge.
Official SDK source and registry release. Use the current package and the matching source ZIP.

Official Rust client for license activation, signed validation, controlled offline
access, feature checks and deactivation. **Rust 1.90+**, SDK **0.4.0**, MIT licensed.

[Documentation](https://licensing.kdpika.com/docs/v1/sdks/rust) · [GitHub source](https://github.com/kdpika/kdpika-licensing-rust) · [Download SDK ZIP](https://github.com/kdpika/kdpika-licensing-rust/releases/download/v0.4.0/kdpika-rust-sdk-0.4.0.zip) · [crates.io package](https://crates.io/crates/kdpika-licensing)

## Install from the source ZIP

Download [kdpika-rust-sdk-0.4.0.zip from KDPika](https://licensing.kdpika.com/docs/downloads/kdpika-rust-sdk-0.4.0.zip)
or the identical versioned asset from GitHub Releases. Verify its SHA-256 against
the [download table](https://licensing.kdpika.com/docs/v1/sdks/overview#download-and-verify),
then extract the `kdpika-rust-sdk` directory beside your application.
Add it to your application's `Cargo.toml`:

```toml
[dependencies]
kdpika-licensing = { path = "../kdpika-rust-sdk" }
```

From your application directory, build with Cargo:

```sh
cargo check
```

Import `kdpika_licensing` in your Rust code. Commit your application's `Cargo.lock`.
The named SDK ZIP has the checksum and directory layout above; GitHub's automatic
**Source code (zip)** is a different repository snapshot.

Rust uses the same API v1 contract as the other SDKs, without depending on Python.
This repository contains only the Rust client, tests and examples, not KDPika's
proprietary SaaS backend. Edition 2021; Rust 1.90 or later is required.

## Install from crates.io

```sh
cargo add kdpika-licensing@0.4.0
```

Or add it to Cargo.toml:

```toml
[dependencies]
kdpika-licensing = "0.4.0"
```

Import it as `kdpika_licensing`. Commit your application's Cargo.lock to retain
the resolved release/dependency versions. To enable the optional async worker:

```sh
cargo add kdpika-licensing@0.4.0 --features async
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

Add the path dependency shown above. Before distributing your application, pin a
reviewed commit and commit the application's Cargo.lock; do not rely on a moving
branch as a release identifier. This repository contains only the Rust client,
its tests and examples, not KDPika's proprietary SaaS backend.

For the SDK's tests, run from the source directory:

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

## SDK repositories and ZIP downloads

| Language | GitHub repository | ZIP from documentation | ZIP from GitHub Releases |
| --- | --- | --- | --- |
| Python | [kdpika-licensing-python](https://github.com/kdpika/kdpika-licensing-python) | [Python 2.0.1 ZIP](https://licensing.kdpika.com/docs/downloads/kdpika-python-sdk-2.0.1.zip) | [Python 2.0.1 ZIP](https://github.com/kdpika/kdpika-licensing-python/releases/download/v1.1.6/kdpika-python-sdk-2.0.1.zip) |
| Rust | [kdpika-licensing-rust](https://github.com/kdpika/kdpika-licensing-rust) | [Rust 0.4.0 ZIP](https://licensing.kdpika.com/docs/downloads/kdpika-rust-sdk-0.4.0.zip) | [Rust 0.4.0 ZIP](https://github.com/kdpika/kdpika-licensing-rust/releases/download/v0.4.0/kdpika-rust-sdk-0.4.0.zip) |
| PHP | [kdpika-licensing-php](https://github.com/kdpika/kdpika-licensing-php) | [PHP 2.0.1 ZIP](https://licensing.kdpika.com/docs/downloads/kdpika-php-sdk-2.0.1.zip) | [PHP 2.0.1 ZIP](https://github.com/kdpika/kdpika-licensing-php/releases/download/v1.0.1/kdpika-php-sdk-2.0.1.zip) |

Each versioned GitHub ZIP asset is byte-identical to the matching documentation
download. GitHub's automatic “Source code (zip)” is a repository snapshot with a
different folder name and may include newer README/CI metadata; use the named
SDK asset when following the documentation's extraction paths and checksum.


## Credit codes and store top-ups

These APIs require the matching credit-commerce server update and WooCommerce connector 0.8.0 or later. An active signed online license remains usable at zero credits. Included features and paid operations are separate.

| Language | Available packs | Redeem a code | Store checkout | History | Publisher-priced job |
|---|---|---|---|---|---|
| Python | `credit_packs()` | `redeem_credits(code, request_key)` | `create_credit_checkout(pack_id)` | `credit_history()` | `consume_action(action, request_key, expected_units)` |
| JavaScript (Node/Electron) | `creditPacks()` | `redeemCredits(code, requestKey)` | `createCreditCheckout(packId)` | `creditHistory()` | `consumeAction(action, requestKey, expectedUnits)` |
| Rust | `credit_packs()` | `redeem_credits(code, request_key)` | `create_credit_checkout(pack_id)` | `credit_history()` | `consume_action(action, request_key, expected_units)` |
| PHP | `creditPacks()` | `redeemCredits(code, requestKey)` | `createCreditCheckout(packId)` | `creditHistory()` | `consumeAction(action, requestKey, expectedUnits)` |
| C# | `CreditPacksAsync()` | `RedeemCreditsAsync(code, requestKey)` | `CreateCreditCheckoutAsync(packId)` | `CreditHistoryAsync()` | `ConsumeActionAsync(action, requestKey, expectedUnits)` |

Persist the request key **before** redeeming or consuming. If the response is lost, explicitly retry that same operation with the same key; never create another charge to recover it. Redemption units and balance are read from the signed response. Open `checkout_url` in the normal browser, never embed a developer API key in an app, and refresh the signed balance after returning. Pack prices are estimates; WooCommerce confirms tax, coupons and the actual payable amount.

`CREDIT_CODE_UNAVAILABLE` covers unknown, expired, exhausted or ineligible codes without exposing another account's promotion. `CREDIT_PRICE_CHANGED` makes the app refresh the price and ask for confirmation. `CREDIT_REVIEW_REQUIRED` blocks paid jobs pending publisher review, not the license's included features. Credit consumption remains online-only.


## Two practical apps — choose how you learn

Both examples remain available; Sudoku Studio does not replace License Lab.

| Example | Open or download | What it teaches |
|---|---|---|
| Original License Lab (PHP) | [Open demo](https://licensing.kdpika.com/license-lab) | Real license activation, signed replies, offline rules, devices and seats |
| Sudoku Studio (PHP web app) | [Open app](https://licensing.kdpika.com/sudoku-studio) · [Source ZIP](https://licensing.kdpika.com/docs/downloads/kdpika-sudoku-studio-php-1.0.0.zip) · [Step-by-step guide](https://licensing.kdpika.com/docs/v1/tutorials/php/sudoku) | A usable puzzle app with exports, plan features, crown balance, codes and store top-ups on the same license |
| Sudoku Studio (Python desktop) | [Source ZIP](https://licensing.kdpika.com/docs/downloads/kdpika-sudoku-studio-python-1.3.0.zip) · [Guide](https://licensing.kdpika.com/docs/v1/tutorials/python/desktop) | A PyQt6 desktop app; App ID, email/key and Live/Test are entered in the window |

![Sudoku Studio PHP application](https://licensing.kdpika.com/assets/sudoku/studio-preview.png)

The web app uses the PHP SDK; the desktop app uses the Python SDK. These are app examples, not claims that this package runs another language. The same credit endpoints are supported by all five SDKs. Enter your own software license in the UI, never a platform administration key. JPG is included, PNG uses `png_export`, collections use `batch_generation`, and paid packs use the publisher-priced `create_pack` action. Promotional code KDP100CR is an example: the publisher creates it before it can be redeemed.

A zero balance keeps included tools available. Real store payment confirmation grants units to the original license once; SDK requests cannot grant themselves credits. GitHub hosts the example source/download links, and this package-registry page includes the same guide.
