## Project structure

- `goldboot/src/builder/os/` — one module per supported OS
- `goldboot/src/builder/options/` — shared config types (`Locale`, `Timezone`,
  `Ntp`, `Packages`, `UnixUsers`, etc.)
- `goldboot-image/` — image format library
- `goldboot-macros/` — proc macros (`#[Os(...)]`, `#[derive(Prompt)]`)

## Adding a new OS

1. Create `goldboot/src/builder/os/<name>/mod.rs`.
2. Annotate the struct with `#[goldboot_macros::Os(architectures(...))]` and
   derive
   `Clone, Serialize, Deserialize, Validate, Debug, SmartDefault, goldboot_macros::Prompt`.
3. Use shared option types from `builder::options` where applicable (hostname,
   locale, timezone, ntp, packages, unix_users, root_password, iso, size, arch).
4. Implement `BuildImage` with a `build()` method. We need to manipulate the VM
   until we get to a shell so we can finish the install via SSH. Use
   `wait_text!` to wait for the given text to be displayed on the screen.
5. Register the module in `goldboot/src/builder/os/mod.rs`.

## Updating for a new upstream OS release

1. **Add the new release variant** to the release enum (e.g. `V3_24`,
   `Bookworm`) at the top of the enum so it sorts newest-first.
2. **Keep EOL releases**
3. **Update the default variant** to the newest stable release.
4. **Update the default ISO** — find the new ISO filename, URL, and sha256
   checksum from the upstream mirror and update the `#[default(Iso { ... })]` on
   the `iso` field.
5. Run `cargo check` to confirm everything compiles.

# Roadmap to 1.0

> This project has been in development for a long time and we need to rapidly
> move toward a MVP and then a stable 1.0 release afterwards. This roadmap
> outlines our overall requirements in no particular order.

- [x] Rename "Provisioners" to "PostSteps" (split into `PreStep`/`PostStep` in
  `builder/steps/`)
- [x] Get ansible PostStep working again
  - PostSteps run over SSH after the initial build has completed (see the
    `nix` builder for the reference wiring)
- [x] Create "PreStep" facility that allows the context directory to be
  customized before build (pre-steps run against an ephemeral copy of the
  context directory — the "effective context dir" — so they're free to make
  changes; the `AnsibleLocal` PreStep renders config templates there)
  - TODO: replace the context-dir copy with an overlayfs mount (behind the
    same `effective_context_dir` abstraction) to avoid copying large contexts
  - TODO: convert ISO (install media) download to a "built in" PreStep?
- Figure out how to package goldboot.efi so it's available to `goldboot install`
- Finish remaining builders
- Finish multiboot builds
