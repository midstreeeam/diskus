# Release

GitHub Actions builds precompiled packages when a version tag is pushed.

1. Update `Cargo.toml`, `Cargo.lock`, `CHANGELOG.md`, and install examples in `README.md`.
2. Run the local checks:

   ```bash
   cargo fmt --check
   cargo clippy --locked --all-targets --all-features
   cargo test --locked
   cargo build --locked --lib --no-default-features
   ```

3. Commit the release prep.
4. Create and push an annotated tag:

   ```bash
   git tag -a v0.9.1 -m "Release v0.9.1"
   git push origin master
   git push origin v0.9.1
   ```

The `CICD` workflow publishes `.tar.gz`, `.zip`, and `.deb` assets to the GitHub release for the tag. End users can download those assets without installing Rust or Cargo.
