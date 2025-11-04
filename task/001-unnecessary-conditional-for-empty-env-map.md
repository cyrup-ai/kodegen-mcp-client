# Task: Remove Unnecessary Conditional for Empty Environment Map

## Location
`src/transports/stdio.rs:224-226`

## Issue Type
- Runtime Performance (minor)
- Code Clarity

## Description
The `build()` method contains an unnecessary conditional check before calling `cmd.envs()`:

```rust
if !self.envs.is_empty() {
    cmd.envs(&self.envs);
}
```

## Problem
1. **Unnecessary Branch**: This adds an extra branch instruction that serves no purpose. Calling `cmd.envs(&self.envs)` with an empty HashMap is a no-op - the Tokio Command implementation will simply not add any environment variables.

2. **Code Clarity**: The conditional suggests there's a meaningful difference between calling `envs()` with an empty map vs not calling it at all, but there isn't. This creates cognitive load for readers who may wonder if there's a subtle behavior difference.

3. **Inconsistency**: The code doesn't have similar checks for `current_dir` (line 228-230), which also could be "optimized" with a similar conditional if this pattern were actually valuable.

## Real-World Impact
- **Performance**: Negligible but unnecessary branch prediction cost on every client creation
- **Maintainability**: Makes code harder to understand and maintain
- **Debugging**: Could confuse developers looking for why env vars aren't being inherited (though this is not actually the issue)

## Root Cause
Likely defensive programming or misunderstanding of how Tokio's `Command::envs()` works. The developer may have assumed calling `envs()` with an empty map would clear the environment or have some other side effect.

## Recommended Fix
Remove the conditional and call `cmd.envs(&self.envs)` unconditionally:

```rust
cmd.envs(&self.envs);
```

Or, if the developer wants to avoid the method call entirely:
```rust
if !self.envs.is_empty() {
    cmd.envs(&self.envs);
}
```
But document WHY this optimization exists (spoiler: it provides no measurable benefit).

## Testing
- Verify that environment variables are still properly set when `self.envs` is not empty
- Verify that parent environment is still inherited when `self.envs` is empty
- Add test case that explicitly checks environment inheritance behavior

## Priority
Low - This is a minor optimization and clarity improvement

## Related Issues
- Task 002: Missing documentation on environment inheritance
- Task 008: No validation that environment variables are being properly set
