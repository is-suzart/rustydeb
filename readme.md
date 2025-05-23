# RustyDeb

## What is RustyDeb?

RustyDeb is a command-line utility, written in Rust, designed to help you install `.deb` packages on Linux systems. While initially inspired by use on Solus OS, the goal is to provide a tool that can understand and process `.deb` files, offering a way to manage packages, especially in environments where direct `.deb` support might be customized or for users who enjoy a hands-on approach to software management.

This project is currently a work-in-progress, born from a learning adventure into the world of Rust and the internals of Linux package management!

## Core Goals & Features (Current & Planned)

RustyDeb aims to provide the following functionalities:

* **`.deb` Archive Extraction:** Securely and accurately unpack `.deb` files, which are `ar` (archive) format.
* **Internal Tarball Processing:** Extract and handle the `control.tar.*` (containing package metadata and maintainer scripts) and `data.tar.*` (containing the actual application files) archives found within each `.deb` package.
* **Metadata Parsing:** Read and interpret the `control` file (from `control.tar.*`) to retrieve essential package information such as its name, version, architecture, description, and dependencies.
* **Installation Manifest Generation:** For each package processed, create a JSON manifest file that records:
    * Key metadata about the package.
    * A comprehensive list of all files that belong to the package.
    * (Planned) A flag for each file indicating whether it can be safely removed by RustyDeb during an uninstallation process.
* **(Next Steps/Future) Package Installation:** Carefully merge/copy the application files from the extracted `data.tar.*` tree into the appropriate locations on the system filesystem (e.g., `/opt`, `/usr`, `/etc`).
* **(Future) Conflict Management:** Implement checks for existing files and potential conflicts before installation.
* **(Future) Uninstallation:** Utilize the generated JSON manifest to provide a clean uninstallation process.
* **(Future) User Interface:** While the initial focus is a robust command-line tool, a simple graphical user interface (perhaps exploring Rust GUI libraries like Iced) is a potential long-term goal.

## How It Works (The Journey So Far)

1.  **Input:** RustyDeb takes the path to a local `.deb` file as a command-line argument.
2.  **Validation:** It performs initial checks to ensure the file exists, is a file, and has a `.deb` extension.
3.  **Temporary Workspace:** A temporary directory is created to handle all extraction operations cleanly.
4.  **AR Extraction:** The outer `.deb` (ar) archive is unpacked into this temporary directory. This typically reveals three main components:
    * `debian-binary`: A small text file indicating the `.deb` format version.
    * `control.tar.*` (e.g., `control.tar.xz`, `control.tar.gz`): An archive containing package metadata and control scripts.
    * `data.tar.*` (e.g., `data.tar.xz`, `data.tar.gz`): An archive containing the actual files that make up the software to be installed.
5.  **TAR Extraction:** Both the `control.tar.*` and `data.tar.*` archives are then extracted into their own dedicated subdirectories within the main temporary workspace (e.g., `control/` and `data/` or a unified `content_data/` directory). This step involves:
    * Identifying the correct compression scheme (GZip, XZ).
    * Using appropriate Rust crates (`flate2`, `xz2`) for decompression.
    * Using the `tar` crate to unpack the tarball contents.
6.  **(Current Focus) Metadata Collection:** (This is what we'll work on next!)
    * Parse the `control` file (from the extracted control archive) to get package name, version, etc.
    * Recursively list all files from the extracted data archive to build the file manifest.
    * Assemble this information into a structured format (likely JSON).

## Technology Stack

* **Language:** Rust
* **Key Crates Used/Planned:**
    * `ar`: For reading `.deb` (ar) archives.
    * `tar`: For reading `.tar` archives.
    * `flate2`: For GZip decompression.
    * `xz2`: For XZ decompression.
    * `tempfile`: For managing temporary directories.
    * `serde` & `serde_json`: For serializing and deserializing the JSON manifest.
    * `clap` (from `std::env::args` initially): For parsing command-line arguments.

---

*This README reflects the current state and aspirations of the RustyDeb project. It's a learning journey, and features are being built step by step!*