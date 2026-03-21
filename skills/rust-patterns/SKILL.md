---
name: rust-patterns
description: Rust patterns Axonix gets wrong repeatedly — check this before writing code.
---

# Rust Patterns — Quick Reference

Check this before writing or editing any Rust code. Each section has Bad/Good
examples. If you recognise the "Bad" pattern in your plan, stop and use the
"Good" version instead.

---

## 1. Ownership and Cloning

### Vec<Box<dyn T>> vs Vec<Arc<dyn T>>

`default_tools()` returns `Vec<Box<dyn AgentTool>>`. Sub-agents need
`Vec<Arc<dyn AgentTool>>`. Convert at the call site, not inside the tool.

```rust
// Bad — Box cannot be shared across sub-agents
let tools = default_tools(); // Vec<Box<dyn AgentTool>>
agent.with_tools(tools);     // moved — can't use again

// Good — convert once, clone cheaply per sub-agent
let arc_tools: Vec<Arc<dyn AgentTool>> = default_tools()
    .into_iter()
    .map(|b| Arc::from(b) as Arc<dyn AgentTool>)
    .collect();
```

### The arc_tools.clone() pattern (from build_tools())

Each sub-agent needs its own `Vec<Arc<dyn AgentTool>>`. Arc is cheap to clone
(increments a reference count). Call `.clone()` once per sub-agent.

```rust
// Bad — moves arc_tools into the first agent; compile error on second use
agent_a.with_tools(arc_tools);
agent_b.with_tools(arc_tools); // E0382: use of moved value

// Good — clone for each sub-agent
agent_a.with_tools(arc_tools.clone());
agent_b.with_tools(arc_tools.clone());
agent_c.with_tools(arc_tools.clone()); // last use can take ownership
```

### Moving vs borrowing in match arms

```rust
// Bad — tries to move out of a reference
let msg = match &response {
    Ok(r)  => r.text,   // E0507: cannot move out of borrowed content
    Err(e) => e.to_string(),
};

// Good — clone or borrow, don't try to move
let msg = match &response {
    Ok(r)  => r.text.clone(),
    Err(e) => e.to_string(),
};
```

---

## 2. Error Handling

### Use ? not unwrap() in production paths

```rust
// Bad — panics in production; unwrap() is a crash waiting to happen
let content = std::fs::read_to_string("file.txt").unwrap();

// Good — propagate with ?; caller decides how to handle failure
fn load(path: &str) -> Result<String, std::io::Error> {
    let content = std::fs::read_to_string(path)?;
    Ok(content)
}
```

### Box<dyn std::error::Error> as return type

Use when the function can return multiple concrete error types.

```rust
// Bad — forces a single concrete error type; breaks when you add a second fallible call
fn run() -> Result<(), std::io::Error> { ... }

// Good — accepts any error type; ideal for top-level or glue functions
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let text = std::fs::read_to_string("cfg.toml")?; // io::Error
    let cfg: Config = toml::from_str(&text)?;         // toml::de::Error
    Ok(())
}
```

### anyhow vs typed errors

```rust
// Use anyhow for binary / application code where you only need to display errors
use anyhow::{Context, Result};
fn start() -> Result<()> {
    let val = risky_op().context("risky_op failed")?;
    Ok(())
}

// Use typed errors (thiserror or plain enums) for library code that callers
// need to match on programmatically
#[derive(Debug, thiserror::Error)]
enum ApiError {
    #[error("rate limited: retry after {0}s")]
    RateLimit(u64),
    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),
}
```

---

## 3. Lifetimes and Closures

### move || vs regular closure

```rust
// Bad — regular closure borrows; spawn requires 'static lifetime
let name = String::from("axonix");
std::thread::spawn(|| println!("{name}")); // E0373: may outlive borrowed value

// Good — move transfers ownership into the closure
let name = String::from("axonix");
std::thread::spawn(move || println!("{name}"));
```

### |x| some_fn(x) vs some_fn alone

Passing a function pointer directly sometimes fails due to lifetime coercion
differences. Using an explicit closure wrapper always works.

```rust
// Bad — compiler cannot coerce the lifetime the trait bound expects
items.iter().map(process_item)  // type mismatch / lifetime error in some contexts

// Good — explicit closure sidesteps coercion ambiguity
items.iter().map(|item| process_item(item))
```

### Capturing references vs owned values

```rust
// Bad — closure captures a reference to data that's dropped before the closure runs
let result = {
    let tmp = expensive_compute();
    move || tmp.value  // E0716 if tmp is a temporary in some forms
};

// Good — assign to a let binding to extend lifetime past the block
let tmp = expensive_compute();
let result = move || tmp.value;
```

---

## 4. Common Compiler Errors — Before/After

### E0382: use of moved value

**What it means:** A value was moved into one place and then used again.

```rust
// Bad
let s = String::from("hello");
let a = s;   // s moved into a
println!("{s}"); // E0382: s already moved

// Fix — clone before moving, or borrow instead
let s = String::from("hello");
let a = s.clone();
println!("{s}"); // s still valid
```

### E0499: cannot borrow as mutable more than once

**What it means:** Two mutable borrows of the same value are alive simultaneously.

```rust
// Bad
let mut v = vec![1, 2, 3];
let a = &mut v;
let b = &mut v; // E0499: already mutably borrowed

// Fix — limit scope of first borrow, or extract work into a method
let mut v = vec![1, 2, 3];
{ let a = &mut v; a.push(4); }  // a's scope ends here
let b = &mut v;                  // now safe
```

### E0716: temporary value dropped while borrowed

**What it means:** A reference points to a temporary that is immediately destroyed.

```rust
// Bad
let r = expensive().result(); // temporary from expensive() dropped at ;
println!("{r}"); // E0716: temporary dropped while borrowed

// Fix — assign the temporary to a let binding first
let tmp = expensive();
let r = tmp.result();
println!("{r}");
```

---

## 5. Cargo and Build Hygiene

### Always run cargo fmt before committing

```bash
# Bad — commit raw unformatted code
git commit -am "feat: add feature"

# Good — format first, then commit
cargo fmt
git add -u
git commit -m "feat(scope): add feature"
```

### Use clippy to catch issues early

```bash
# Minimum: run clippy treating warnings as errors
cargo clippy --all-targets -- -D warnings

# Clippy catches: unnecessary clones, redundant closures, unwrap() in library code, etc.
```

### Adding a dependency

```bash
# Bad — edit Cargo.toml manually and forget to rebuild
echo 'serde = "1"' >> Cargo.toml  # Cargo.lock not updated

# Good — use cargo add (updates Cargo.toml and Cargo.lock atomically)
cargo add serde --features derive
# or: edit Cargo.toml then immediately run
cargo build  # updates Cargo.lock
```

### #[allow(dead_code)] is a code smell

```rust
// Bad — silences the warning without fixing the problem
#[allow(dead_code)]
fn helper_never_used() { ... }

// Good — remove unused code, or wire it into a real call path
// If it will be used soon, add a TODO comment and a tracking issue instead
```

---

*Written by Axonix — G-036 / Issue #42*
