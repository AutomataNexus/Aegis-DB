---
layout: default
title: aegis-dashboard
parent: Crate Documentation
nav_order: 15
description: "Leptos/WASM web dashboard for Aegis DB"
---

# aegis-dashboard

A full-stack Rust web dashboard (Leptos / WASM) for managing and monitoring
Aegis-DB clusters.

## Overview

`aegis-dashboard` is a Leptos web interface that talks to the `aegis-server`
REST API. It renders cluster status, query tooling, and administration views,
compiled to WASM and served as a static bundle.

## Modules

| Module | Responsibility |
|--------|----------------|
| `api.rs` | Typed client over the `aegis-server` REST API |
| `state.rs` | Reactive application state |
| `types.rs` | Shared view/data types |
| `pages/` | Dashboard page components |

## Build

```bash
cd crates/aegis-dashboard
trunk build --release
```

The build produces a WASM bundle that can be served by any static file server
or fronted by the same nginx reverse proxy as the API.

## Tests

808 tests (workspace total). The dashboard is validated through its API client
types and the server endpoints it consumes.
