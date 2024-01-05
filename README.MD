# Jailer

**Project status & info:**

|                    License                     |              Crates Version               |                 Docs                 |
| :--------------------------------------------: | :---------------------------------------: | :----------------------------------: |
| [![License: MIT][license_badge]][license_link] | [![Crate][cratesio_badge]][cratesio_link] | [![Docs][docsrs_badge]][docsrs_link] |

Crate to create sandbox environment for testing and other use cases 


## Add as dependencies

Edit Cargo.toml file to add jailer as dependencies

```toml
[dependencies]
jailer = "0.1.0"
```

# Usage

To use jailer at first create jailer

```rust
let _jailer = jailer::Jailer::new();
// use rust code as required
```


After creation of jailer you can use std env and directory as required and when jailer gets dropped all folder contents and environment variable gets replaced

[license_badge]: https://img.shields.io/github/license/iamsauravsharma/jailer.svg?style=for-the-badge
[license_link]: LICENSE
[cratesio_badge]: https://img.shields.io/crates/v/jailer.svg?style=for-the-badge
[cratesio_link]: https://crates.io/crates/jailer
[docsrs_badge]: https://img.shields.io/docsrs/jailer/latest?style=for-the-badge
[docsrs_link]: https://docs.rs/jailer
