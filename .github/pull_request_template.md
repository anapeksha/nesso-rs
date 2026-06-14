# Pull Request

## What does this PR do?

<!-- One paragraph. What changed and why. Not how — the diff shows how. -->

## Is there anything you're unsure about?

<!-- Optional. Flag anything you want extra eyes on. -->

---

## AI Slop Review Checklist

**Complete this before requesting review. Every unchecked box needs a comment explaining why it doesn't apply.**

### Comments

- [ ] No comment restates what the code already says (`// Initialize the delay` above `Delay::new()`)
- [ ] No `// This unwrap is safe because...` — use `expect("reason")` or restructure
- [ ] No commented-out code blocks
- [ ] No `// TODO` on code that is already implemented
- [ ] Doc comments explain *why* or *what to watch out for*, not just *what the function is named*
- [ ] Hardware timing constants reference a datasheet, register name, or measured reason
- [ ] No wall of comments above a trivial function

### Naming

- [ ] No error variant named `SomethingFailed`, `InitializationError`, `ConfigurationError`, or `InvalidInput` — must name the specific failure
- [ ] No variable named `result`, `data`, `value`, `temp`, or `val` without qualification
- [ ] No function named `handle_X`, `process_X`, or `do_X` — name the actual behavior
- [ ] No magic numbers inline — every hardware literal is a named constant

### Structure

- [ ] No module containing exactly one struct and nothing else
- [ ] No file that is only re-exports (flatten unless feature-gated)
- [ ] No builder pattern on a struct with fewer than 5 fields and no invariants to enforce
- [ ] No newtype wrapper with zero methods and zero validation
- [ ] No function longer than ~50 lines without a clear reason

### Error Handling

- [ ] No bare `unwrap()` outside `#[cfg(test)]`
- [ ] No `.map_err(|e| SomeError::Thing(e))` where a `From` impl would do
- [ ] No `panic!` in library code paths
- [ ] Errors carry enough context to act on without reading source

### AI-Specific Tells

- [ ] No function that exists solely to call one other function with identical arguments
- [ ] No enum with an `Other` or `Unknown` variant unless genuinely needed
- [ ] No `Default` impl that is just `Self::new()` with no arguments and no explanation
- [ ] No trait impl where any method body is `todo!()` or `unimplemented!()`
- [ ] No `#[allow(dead_code)]` without a comment explaining why the code is kept
- [ ] No identical error-handling block copy-pasted across 3+ functions
- [ ] No `pub use` re-export chain longer than 2 levels deep
- [ ] No struct field named `is_initialized`, `has_been_set`, or `was_called`

### Examples

- [ ] No bare `unwrap()` in any example — use `expect()` or `-> Result<>`
- [ ] Examples show realistic usage, not just "call every public method once in sequence"
- [ ] Hardcoded values (serial ports, credentials, frequencies) are clearly marked with a comment
- [ ] New hardware features have at least one example

### Tests

- [ ] No test named `test_it_works`, `test_basic`, `test_simple`, or `test_foo`
- [ ] No test that only asserts `result.is_ok()` with no content check
- [ ] Test names describe the scenario and expected outcome, not just the function under test
- [ ] Host-runnable tests added for any logic that doesn't require hardware
- [ ] `HARDWARE_VALIDATION.md` updated if new hardware paths were added

### Docs

- [ ] README install snippet matches the version in this PR if version changed
- [ ] `CHANGELOG.md` has an entry for anything user-visible
- [ ] No `# Errors` doc section that just says "Returns an error if something goes wrong"
- [ ] No `# Panics` doc section that omits the actual condition

### CI Gates (must all be green before review is requested)

- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes
- [ ] `cargo fmt --check --all` passes
- [ ] `cargo build --workspace --release` passes
- [ ] `cargo xtask lint` passes (see `xtask/` for what this checks)
- [ ] No new `#[allow(clippy::...)]` suppressions without a justifying comment

---

**Reviewer note:** a cluster of minor checklist failures in the same function is a single major failure — the whole function was generated and not read critically. Request a full rewrite of that scope, not a patch.
