![rustc](https://img.shields.io/badge/rustc-1.85.0-blue.svg)
[![codecov](https://codecov.io/gh/averageeucplayer/lost-metrics-store/graph/badge.svg?token=HHRGYYUNM2)](https://codecov.io/gh/averageeucplayer/lost-metrics-store)
![CI](https://github.com/averageeucplayer/lost-metrics-store/actions/workflows/ci.yml/badge.svg)

# 💾 Lost Metrics Store

A library for managing and storing encounter data in a SQLite database.

This crate provides efficient data handling with support for querying, inserting, and filtering stored encounters.

It is designed for seamless integration with other applications that require structured encounter tracking and retrieval.

## 📦 Installation & Setup

### 1️⃣ **Clone the Repository**

```sh
git clone https://github.com/averageeucplayer/lost-metrics-store.git
```

### 2️⃣ Add to Cargo.toml

```toml
[dependencies]
lost-metrics-store = { git = "https://github.com/averageeucplayer/lost-metrics-store" }
```