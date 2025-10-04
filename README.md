# 🦀 Rust SFTP Client Example

This repository contains a **basic example of an SFTP client implementation in Rust**, built with:

- [`openssh`](https://crates.io/crates/openssh) – for managing SSH sessions  
- [`openssh-sftp-client`](https://crates.io/crates/openssh-sftp-client) – for SFTP file transfer operations  

The example demonstrates how to perform the most common SFTP operations programmatically:

- ✅ Connect to a remote server  
- ✅ List remote directories (`ls`)  
- ✅ Upload files (`put`)  
- ✅ Download files (`get`)  
- ✅ Disconnect from the server  

---

## 📖 Purpose

The goal of this repository is to provide **clear, working Rust code** showing how to integrate SFTP functionality into your own projects.  
It is **not a production-ready library or CLI tool**, but a **learning resource and starting point** for developers who want to embed SFTP into their applications.

Read the full API documentation: `cargo doc --open`

---

## 📦 Requirements

- Rust (latest stable recommended)  
- An accessible SSH/SFTP server for testing  

---

## ▶️ Example Usage

Examples are provided under the directory `examples`. 
You can run it directly with `cargo run` after adjusting the host/user/path values inside the file.

### Run basic example
cargo run --example basic_usage

### Run advanced example
cargo run --example advanced_usage

## Planned feature addition:
- Add operation progress tracker
- Add support to pass status tracking handlers in the operation apis
- Add support for delete/mkdir
- Rest API addition 
- Containerization of the project for easy testing
