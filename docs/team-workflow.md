# Team workflow (nommsta/VIctory)

This fork has two contributors. Each person works on a different GPU vendor.
These rules prevent one person's work from overwriting the other's.

| Person | GitHub | GPU | Branch prefix |
|--------|--------|-----|---------------|
| nommsta | `nommsta` | AMD (RDNA) | `amd/` |
| Lachlan | `Vapour38` | NVIDIA (RTX 5070 Ti, Blackwell) | `nvidia/` |

## Remotes

| Remote | Repository | Use |
|--------|------------|-----|
| `origin` | `nommsta/VIctory` | our shared fork; the only place we push |
| `upstream` | `Marvisak/VIctory` | the base project; read and merge only |

These repositories are references. Do not push to them:

* `xcl-dev/xcl`: a different layer for the same platform (GPL-2.0-only).
* `fxceless/victory-fork`: a VIctory fork with world-rendering and NVIDIA fixes.
* `Zettrox/ztrx`: a VIctory fork for NVIDIA, based on `fxceless/victory-fork`.

## Rule 1: the asset policy

Read [asset-policy.md](asset-policy.md). Nothing from the game goes into git.
Run `cargo xtask install-hooks` once after you clone.
Keep the game files, caches and logs outside the repository.

## Branches

1. Do not push to `main` directly. Change `main` only through a pull request.
2. Do not force-push `main`. Do not rewrite the history of `main`.
3. Put your work on a branch with your prefix: `nvidia/<topic>` or `amd/<topic>`.
4. Do not push to, rebase or delete a branch with the other person's prefix.
5. Use `shared/<topic>` for a change that is not specific to one vendor.
   The other person must approve a `shared/` pull request before the merge.
6. Use `sync/upstream-<yyyy-mm-dd>` to bring `upstream/main` into `main`.
   Merge it. Do not rebase it. Tell the other person before you start one.
7. Before you open a pull request, merge the current `origin/main` into your branch.

## Vendor-specific code

The AMD path and the NVIDIA path share most of the code. Most conflicts will
occur in `crates/victory-gpu/src/` and `crates/victory-shader/src/emit/lower.rs`.

1. Select a vendor path at runtime from the device capabilities.
   Use `WaveLaneCountMin` / `WaveLaneCountMax` from `D3D12_FEATURE_D3D12_OPTIONS1`
   before the adapter `VendorId` (`0x10DE` NVIDIA, `0x1002` AMD).
2. Do not use a Cargo feature or `cfg` to select a vendor. One build must run on both.
3. A change for one vendor must not change the output on the other vendor.
   If it must, use the `shared/` prefix and get approval.
4. Put new vendor-specific code in its own module when possible.
   This keeps the edits to the large shared files small.
5. Tell the other person before you make a large edit to `lower.rs` or `victory-gpu/src/host.rs`.
6. Name new switches `VICTORY_<NAME>`. Write a comment where the code reads each switch.

## Code from other projects

| Source | Licence | What we can do |
|--------|---------|----------------|
| `Marvisak/VIctory` and its pull requests | GPL-3.0-or-later | merge or cherry-pick |
| `fxceless/victory-fork`, `Zettrox/ztrx` | GPL-3.0-or-later | cherry-pick; credit the author in the commit body |
| `xcl-dev/xcl` | GPL-2.0-only | reference only; write the code again in VIctory's structure |
| WinDurango | LGPL-3.0 | do not use |

GPL-2.0-only code cannot go into a GPL-3.0-or-later project. Do not copy XCL code.
You can read XCL to learn how a problem was solved, then write a new solution.

## Commits and pull requests

* Use the upstream subject style: `<Area>: <what the change does>`.
  Areas in use: `GPU`, `Shaders`, `Loader`, `HLE`, `Audio`, `Input`, `Formats`, `Tests`, `Docs`.
* Write one logical change per commit and per pull request.
* A bug fix includes a test that fails without the fix.
* Before a pull request, run these commands and paste the results:

```powershell
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --locked
```

* For a GPU or shader change, also name the GPU, the driver version and the in-game result.
* Do not add AI attribution lines to commits or pull requests.
