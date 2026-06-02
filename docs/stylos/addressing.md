# Stylos Addressing

## Identity

Every stylos process claims an identity tuple `(realm, role, instance)`
where each segment matches `[a-z0-9][a-z0-9-]*` (lowercase alphanumeric
plus hyphen, must not start with a hyphen). Validation lives in
`stylos-identity`; a bad segment fails `StylosConfig::to_identity()` before
the session is opened.

- **realm** — logical partition ("env"): `dev`, `prod`, `lab`. Two realms
  never exchange traffic at the application layer.
- **role** — what this process *is*: `watcher`, `cli`, `mailbox`.
- **instance** — per-process id: `host-a-42`, a short UUID, etc.

The identity root key is mechanical:

```
stylos/<realm>/<role>/<instance>
```

For example: `stylos/dev/watcher/host-a-42`.

## Key expressions

Stylos uses raw zenoh key expressions under the `stylos/...` namespace.
Wildcards follow zenoh semantics:

| Expression                       | Matches                                       |
| -------------------------------- | --------------------------------------------- |
| `stylos/dev/watcher/host-a-42`   | A specific instance's root                    |
| `stylos/dev/watcher/*`           | All watchers in `dev`                         |
| `stylos/dev/*/*/status`          | Status sub-key of every role in `dev`         |
| `stylos/**`                      | Entire stylos namespace (avoid in production) |

## Stele mailbox keys

Each stele-server node (role `stele`, instance `{instance}`) serves the
postal mailbox ([PRD-027](../prd/prd-027-postal-mailbox.md), v0.20.0) under
its own instance root:

```
stylos/{realm}/stele/{instance}/mailbox/{leaf}
```

where `{leaf}` ∈ `send | list | read | archive | get | outbox | deliver`.
All seven are zenoh **queryables** — JSON request/reply with a uniform
`{ "ok": true, … }` / `{ "ok": false, "error", "reason" }` envelope.
`deliver` is the **only cross-node leaf** (node→node inbound delivery); the
other six are the **local client API**, issued only by the host's own
transient `stele mail` CLI peer.

Senders resolve `{instance}` for a destination host via the `info`
queryable's `mailbox_host` field — a wildcard GET on
`stylos/{realm}/stele/*/info` yields `{instance, mailbox_host}` pairs, and
the sender picks the instance whose `mailbox_host` matches the target host
(case-insensitive, §4.2). `mailbox_host` honours `STELE_HOST` > hostname.

This is standard-compliant: the role stays `stele` and the instance stays
`{instance}` — the mailbox leaves live *below* the instance root. Do not
overload the instance slot to carry mailbox routing.

## POC keys

The runnable POC (PRD-019 §4.8) uses three keys:

```
stylos/dev/poc/rust   # Rust peer publishes
stylos/dev/poc/go     # Go peer publishes (Pass B-Go)
stylos/dev/poc/echo   # queryable endpoint; either peer can get
```
