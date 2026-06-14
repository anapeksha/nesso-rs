# Reviewing nesso-rs

This document is for anyone reviewing a pull request — human or AI-assisted.
The PR template checklist covers the mechanical items. This covers the judgment
calls that automation cannot make.

---

## What CI already enforces

Before a PR can merge, CI runs:

- `cargo fmt --check --all` — formatting
- `cargo clippy --workspace --all-targets -- -D warnings` — lint
- `cargo xtask lint` — AI slop pattern checks (see below)
- `cargo doc --workspace --no-deps` with `-D warnings` — doc completeness
- `cargo test --workspace --lib` — host-runnable tests

If any of these fail the PR cannot merge. Do not review a PR with a red CI
status unless you are diagnosing the failure.

---

## What `cargo xtask lint` checks

The xtask linter scans every `.rs` file in the workspace for:

| Rule | What it catches |
|---|---|
| `no-bare-unwrap` | `.unwrap()` outside `#[cfg(test)]` |
| `no-restating-comment` | Comments that restate what the code says |
| `no-safe-unwrap-comment` | "this unwrap is safe because..." comments |
| `no-todo-on-implemented-code` | `todo!()` / `unimplemented!()` in library code |
| `no-placeholder-comments` | `TODO` / `FIXME` / `XXX` placeholder comments |
| `no-placeholder-expect` | placeholder `expect(...)` messages |
| `no-vague-error-variants` | `InitializationError`, `ConfigurationError` etc. |
| `no-vague-variable-names` | `let result`, `let data`, `let value` etc. |
| `no-handle-process-do-functions` | `fn handle_X`, `fn process_X`, `fn do_X` |
| `no-panic-in-library` | `panic!()` in non-test library code |
| `no-is-initialized-fields` | `is_initialized`, `was_called` struct fields |
| `no-allow-dead-code-without-comment` | `#[allow(dead_code)]` without explanation |
| `no-vague-test-names` | `test_it_works`, `test_basic` etc. |
| `no-ok-only-test-assertion` | `assert!(result.is_ok())` with no content check |
| `no-single-item-module` | Files with one type and nothing else |
| `no-copy-pasted-error-blocks` | Identical error-handling blocks 3+ times |

Adding a suppression with `#[allow(...)]` requires a comment on the preceding
line explaining why. The xtask will flag unsuppressed `#[allow(dead_code)]`
without a comment.

---

## Judgment calls that automation cannot make

### The density rule

A single vague variable name in a 200-line file is noise. Five vague variable
names in a single function means the function was generated and not read. Treat
a cluster of minor checklist items in the same scope as one major failure.
Request a rewrite of that scope, not a patch of each item.

### Comments that sound like documentation but aren't

Automation catches obvious restatement. It cannot catch comments that *sound*
thorough but add nothing:

```rust
// Bad — sounds informative, says nothing
// This is the main entry point for board initialization.
// It sets up all the peripherals and returns a Nesso struct.
pub fn new(peripherals: Peripherals) -> Result<Self, NessoError> {
```

Ask: would a reader who already understands Rust and embedded systems learn
anything from this comment that they could not learn from the signature alone?
If not, the comment should go.

### Error variants that name the subsystem but not the failure

The xtask catches the most egregious names. It will not catch:

```rust
// Passes xtask lint, still bad
DisplayError::SpiTransferFailed   // what transfer? what failed?
LoraError::RadioError             // what about the radio?
WifiError::StackError             // what stack? what kind of error?
```

Good error variants tell you what the caller should do differently:

```rust
DisplayError::ResetPinUnavailable
LoraError::FrequencyOutOfBand { requested_hz: u32 }
WifiError::CredentialsTooLong { max_len: usize }
```

### Examples that work but mislead

An example that compiles and runs but models bad practice is worse than no
example. Watch for:

- Examples that call every public method in sequence with no real task
- Examples that hardcode credentials or device paths without marking them clearly
- Examples that use `expect("TODO")` — the placeholder is the slop
- Examples that show the happy path only with no indication of what can fail

### The "just in case" pattern

AI tends to add fields, parameters, and return types that aren't needed yet
"for flexibility". These accumulate into an API that no one fully understands.

```rust
// Bad — config field that nothing reads
pub struct LoraConfig {
    pub frequency: u32,
    pub spreading_factor: u8,
    pub reserved_for_future_use: u8,  // no one sets this, nothing reads it
}
```

If a field has no caller and no reader, remove it. It can be added when it is
needed with a real name.

---

## Adding a new rule to `cargo xtask lint`

1. Add a `Rule` entry to the `RULES` slice in `xtask/src/lint.rs`
2. Give it a kebab-case `id`, a human-readable `description`, and a
   `check` closure that takes a line and returns `bool`
3. Set `skip_in_tests: true` if the pattern is acceptable inside
   `#[cfg(test)]` blocks
4. Add a row to the table in this file
5. Open a PR with at least one example of the violation the rule catches

---

## When to reject a PR outright vs. request changes

Request changes (do not reject) when:

- Individual items from the checklist are missing but the overall structure is sound
- A single function or module needs to be rewritten

Reject (close without merging) when:

- The majority of new code was generated and not read before submission
- The same vague patterns appear across more than three unrelated scopes
- Public API was changed without a CHANGELOG entry
- CI is red and the author has not investigated

A rejection is not a judgment on the author. It is a signal that the
contribution needs a different approach before it is ready for review.
