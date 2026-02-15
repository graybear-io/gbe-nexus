# Open Issues for Re-import

Captured before project restructure (2026-02-15).

## Bridge adapter between gbe and gbe-transport

- **Original ID**: gbe-transport-07d
- **Type**: task
- **Priority**: P3
- **Owner**: Mike Taylor
- **Status**: open (deferred)
- **Created**: 2026-02-14

### Notes

Thin GBE adapter that subscribes to gbe-transport streams and bridges into
the local gbe tool composition layer (Unix sockets). Different layers, no
code overlap. Deferred until both projects have stable interfaces.

See also: [closed-issues.md](closed-issues.md)
