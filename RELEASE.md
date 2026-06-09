# Release

GitHub Actions builds precompiled packages for pull requests, `master`, and version tags.
After a successful `master` build, the workflow publishes a release for the current
`Cargo.toml` version if that release does not already exist.

1. Update `Cargo.toml`, `Cargo.lock`, `CHANGELOG.md`, and install examples in `README.md`.
2. Run the local checks:

   ```bash
   cargo fmt --check
   cargo clippy --locked --all-targets --all-features
   cargo test --locked
   cargo build --locked --lib --no-default-features
   ```

3. Commit and merge the release prep to `master`. The `CICD` workflow will publish
   the corresponding `vX.Y.Z` release automatically if that release does not already exist.

Manual releases are still supported by creating and pushing an annotated tag:

   ```bash
   git tag -a v0.9.2 -m "Release v0.9.2"
   git push origin master
   git push origin v0.9.2
   ```

The `CICD` workflow publishes `.tar.gz`, `.zip`, and `.deb` assets to the GitHub release. End users can download those assets without installing Rust or Cargo.
