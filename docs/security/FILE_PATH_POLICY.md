# File and path policy

Brief 05 implements one local, provider-independent Rust policy in `crates/privacy`. Its version is `file-path-v1`. Every result has a decision, a fixed safe reason code, an optional project-relative storage path, and the policy version. The result never embeds rejected path text or file content.

## Decisions and precedence

| Decision | Meaning | Stored path |
|---|---|---|
| `Allow` | A later, separately authorized and sanitized bounded read may include content. | Project-relative path |
| `MetadataOnly` | Keep project-relative change metadata; never include file content. | Project-relative path |
| `Deny` | Do not read content or retain the path spelling; count by safe reason only. | None |

Precedence is: built-in security deny → filesystem/root safety deny → user exclusion deny → built-in metadata-only → size restriction → allow. User rules only narrow access. A `!` line in `.mochiignore` has no re-allow effect, including for `.env`.

## Defaults

Secret-bearing basenames are denied case-insensitively: `.env` and `.env.*`, PEM/key/certificate stores (`*.pem`, `*.key`, `*.p12`, `*.pfx`, `*.jks`), private SSH key names, names beginning with `credentials`, `secret.*`, and `secrets.*`. `.ssh`, `.aws`, `.azure`, `.gnupg`, `.kube`, `.codex`, and `.git` directory contents are denied. SQLite/database files (`*.sqlite`, `*.sqlite3`, `*.db`, and SQLite sidecars) are denied. These rules match complete names, intentional prefixes, or extensions, not innocent substrings such as `secretary.ts`.

Dependency, build, coverage, framework-cache, and vendor trees (`node_modules`, `target`, `dist`, `build`, `coverage`, `.next`, `.nuxt`, `.cache`, `cache`, `vendor`) are denied and counted. `generated/`, minified assets and source maps are metadata-only. Known binary extensions are metadata-only; bounded byte samples containing NUL or invalid UTF-8 are also metadata-only. No OCR, archive inspection, or binary extraction occurs. `pnpm-lock.yaml`, `package-lock.json`, `yarn.lock`, and `Cargo.lock` are metadata-only. Ordinary manifests such as `package.json`, `Cargo.toml`, `pyproject.toml`, and `requirements.txt` remain eligible for bounded content reads.

The maximum eligible file size for capture and Git content is 64 KiB; future analysis content is limited to 32 KiB. Greater size yields metadata-only with `FileTooLarge`. Unknown size does not authorize unbounded reading: callers must re-evaluate when metadata is available, enforce bounded reads, and recheck policy before content use. No policy path truncates silently. File-size metadata inspection itself does not load file content.

## Roots, links, and storage

The approved project root is canonicalized once when loading the policy. Candidate separators and `.`/`..` components are normalized before matching. Any lexical escape or absolute path outside the root is denied. The original root spelling is retained locally as an alias to handle macOS `/var` versus `/private/var`; it is not returned in results. Every existing path component is checked without following links. Any symlink, special file, or unsafe filesystem error fails closed. Deleted or not-yet-created paths still receive lexical classification. Rules are matched case-insensitively even on a case-sensitive filesystem to protect case-insensitive macOS volumes. Relative path case is preserved in allowed metadata.

No outside-root full path is stored by this policy. A denied outside path is represented only by `OutsideProject` and a count; no filename is necessary. Git snapshots retain only approved relative paths, and use `[PROJECT_ROOT]` for the repository root. Future outbound analysis must independently enforce consent and sanitize those relative paths.

## `.mochiignore`

The optional root `.mochiignore` is local policy configuration; its content is never learning context. It is read only as a regular, non-symlink file, with a 16 KiB / 256-rule cap. Blank lines and `#` comments are ignored. Supported patterns are simple Gitignore-like `*`, `**`, `?`, directory suffix `/`, and root prefix `/`; basename patterns match at any depth, while patterns containing `/` match from the root. Backslash escapes, parent traversal, and unsupported syntax fail loading conservatively. `!` negation lines are ignored, never used to widen built-in or earlier restrictions. The file itself is denied as learning evidence. A missing ignore file is valid; unreadable or malformed policy prevents Git collection and causes provider file-tool content to be discarded.

## Integration and failure behavior

Capture normalization checks structured file-tool input paths, including patch headers, before retaining tool response text. If paths are denied, metadata-only, missing, oversized, ambiguous, or policy loading fails, it keeps only the tool completion metadata. The existing text sanitizer still runs on retained content. This is a bounded guard for the current prototype adapter, not a claim to understand arbitrary shell commands. Command-output semantic filtering and comprehensive secret redaction belong to Brief 06.

Git applies the same policy to tracked, untracked, deleted, and rename paths before any working-tree file read. On Unix, the file open refuses a final symlink, then verifies the opened file identity against a path still inside the root before a bounded read. Denied paths contribute only to `excluded_count`. Metadata-only files keep a relative path and omission code; no content hash is created. The reader does not request a full repository diff or excluded Git object. Policy errors fail the snapshot rather than allow an unfiltered read.
