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

## POC keys

The runnable POC (PRD-019 §4.8) uses three keys:

```
stylos/dev/poc/rust   # Rust peer publishes
stylos/dev/poc/go     # Go peer publishes (Pass B-Go)
stylos/dev/poc/echo   # queryable endpoint; either peer can get
```
