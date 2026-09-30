## What and why

<!-- One or two sentences. Link the issue if there is one. -->

## Checks

- [ ] `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all`
- [ ] `npx tsc --noEmit -p . && npm test && npm run build`
- [ ] Tests added or updated for what changed
- [ ] Still local only: no network calls, accounts or telemetry
- [ ] No personal data (names, emails, gamer tags, game account IDs) in code, tests or screenshots
- [ ] Design names are our own (no franchise, character or brand names)
- [ ] No hardware writes outside the rules in docs/PROTOCOL.md (*Timing*); tried on the simulator first
