# Jailer

**Project status & info:**

|                    License                     |              Crates Version               |                 Docs                 |
| :--------------------------------------------: | :---------------------------------------: | :----------------------------------: |
| [![License: MIT][license_badge]][license_link] | [![Crate][cratesio_badge]][cratesio_link] | [![Docs][docsrs_badge]][docsrs_link] |

Crate to create a sandbox environment for testing and other use cases. It provides an isolated temporary directory that automatically cleans up upon closure or when the object goes out of scope.


## Add as dependencies

Edit Cargo.toml file to add jailer as dependencies

```toml
[dependencies]
jailer = "0.2.0"
```

# Usage

Jailer allows you to create a temporary directory where all operations occur. Once the jailer instance is dropped or closed, the directory and its contents are automatically deleted.

```rust
let mut jailer = jailer::Jailer::new().unwrap();
// use rust code as required
// Data is deleted when jailer is dropped or when it is closed
jailer.close().unwrap();
```

### Using EnvJailer for Environment Variable Isolation

EnvJailer extends Jailer by also managing environment variables. It ensures that environment changes within the jail do not persist beyond its lifetime.
```rust
use jailer::EnvJailer;
use std::env;

let mut env_jailer = EnvJailer::new().unwrap();
unsafe {
    env_jailer.set_env("TEST_KEY", "sandboxed_value");
}
assert_eq!(env::var("TEST_KEY").unwrap(), "sandboxed_value");
unsafe {
    env_jailer.close().unwrap();
}
assert!(env::var("TEST_KEY").is_err());
```

[license_badge]: https://img.shields.io/github/license/iamsauravsharma/jailer.svg?style=for-the-badge
[license_link]: LICENSE
[cratesio_badge]: https://img.shields.io/crates/v/jailer.svg?style=for-the-badge
[cratesio_link]: https://crates.io/crates/jailer
[docsrs_badge]: https://img.shields.io/docsrs/jailer/latest?style=for-the-badge
[docsrs_link]: https://docs.rs/jailer
