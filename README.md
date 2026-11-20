# Memory Analyzer

A fast Windows memory analyzer written in Rust, developed as a diploma project.

## Features

* **Fast Memory Scanning:** Utilizes SIMD instructions (AVX2 / AVX-512) for rapid pattern matching.
* **Live Memory Editing:** Read and write values directly to target process memory spaces.
* **Process Inspection:** Attach to running Windows processes and inspect memory regions.

## Requirements

- **OS:** Windows 11 (build `26100.4770` or newer)
- **CPU:** x86_64 with **AVX2** or **AVX-512F** (fast **BMI2** recommended)
- **Toolchain:** `rustc` nightly `1.98.0` (`2026-08-27`) or newer, plus `cargo`

## Build & Install

There are no prebuilt releases yet - build from source:

```powershell
mkdir memedit
cd memedit
git clone git@github.com:WannaCry1LoL/Diploma-Project.git .
cargo build --release
```

## Disclaimer

This software was created strictly for educational and research purposes only. Use it solely on software that you own or have explicit authorization to analyze.
