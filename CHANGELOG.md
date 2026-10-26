# Current release: 0.2.0

- Required signed hardware binding, floating checkout/heartbeat/checkin and online idempotent usage credits.
- Updated installation, source links and cross-language capability documentation.
- Customer portal remains hosted; browser hardware fallback is unsupported.

# Changelog

## 0.2.0 — 2026-10-26

- Require product-salted hardware hashes and verify signed device/installation binding.
- Add floating checkout, heartbeat and checkin with signed lease expiry and seat exhaustion.
- Add online-only credit balance/consume, idempotent receipts and insufficient-credit errors.
- Breaking pre-launch server contract; older published clients are not compatible.
- Registry availability is recorded separately; a changelog is not proof of publication. Ed25519 is unchanged.

## Unreleased — Phase B/C/D parity

- Required native hardware fingerprint and signed device binding; no browser fallback.
- Automatic floating checkout/heartbeat, explicit checkin/close, signed lease gating
  and ALL_SEATS_IN_USE; synchronous and async clients.
- Online-only consume/get_balance, authenticated idempotent receipts and typed
  insufficient-credit refusal. No Ed25519, dependency or registry changes.

## 0.1.1 — 2026-10-24

Documentation-only release: prominent documentation, GitHub and versioned ZIP
links; source ZIP installation steps alongside Cargo installation. Runtime code,
dependencies, API behavior and license terms are unchanged from 0.1.0.

## 0.1.0 — 2026-10-24

Initial official Rust SDK source release. Native synchronous client, optional
bounded async worker, API v1 activation/validation/deactivation, strict Ed25519
and PHP canonical JSON compatibility, persistent installation identity, atomic
private file storage with cross-process locks, signed offline grace and safe
feature/limit helpers. Registry publication is separate from source delivery.
