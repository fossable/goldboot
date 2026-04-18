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

### Pop!\_OS (`goldboot/src/builder/os/pop_os/mod.rs`)

Release page: https://pop.system76.com ISO index:
`https://iso.pop-os.org/<YY.MM>/amd64/<generic|nvidia>/<revision>/` API:
`https://api.pop-os.org/builds/<YY.MM>/stable?arch=amd64`

**Release enum** — `PopOsRelease` lists LTS releases only, newest-first.
Pop!\_OS follows Ubuntu's LTS cadence.

Pop!\_OS uses a **custom graphical installer** (not Ubuntu autoinstall).
Installation is driven by VNC automation in `build()`. Because of this, the
primary user account must be configured via `user: PopOsUser` — the installer
requires it. Post-install config (extra users, packages, hostname, timezone) is
applied over SSH.

### Ubuntu (`goldboot/src/builder/os/ubuntu/mod.rs`)

Release page: https://ubuntu.com/about/release-cycle ISO index:
https://releases.ubuntu.com/<codename>/SHA256SUMS

**Release enum** — `UbuntuRelease` lists only currently-supported releases. LTS
releases are preferred; interim releases are included but not the default.

Ubuntu uses **autoinstall** (cloud-init YAML) for unattended installation —
generated at build time in `Ubuntu::generate_autoinstall()`. There is no static
config file.

### Debian (`goldboot/src/builder/os/debian/mod.rs`)

Release page: https://www.debian.org/releases/ ISO index:
https://cdimage.debian.org/cdimage/release/current/amd64/iso-cd/

**Release enum** — `DebianEdition` lists releases stable-first (Trixie,
Bookworm, Bullseye, Forky, Sid). Stable and oldstable always have real ISOs;
testing/unstable are opt-in.

The preseed is generated at build time in `Debian::generate_preseed()` — no
static file to update.

### Alpine Linux (`goldboot/src/builder/os/alpine_linux/mod.rs`)

Release page: https://alpinelinux.org/releases/ CDN root:
https://dl-cdn.alpinelinux.org/alpine/

**Release enum** — `AlpineRelease` lists supported branches newest-first, with
`Edge` last. Only include branches that appear on the releases page as actively
supported.

## TODO list

- Finish registry implementation
- Create nixpkgs branch for configuring server
- Figure out how to package goldboot.efi
- Finish remaining builders
- Finish multiboot builds
