# Changelog

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
