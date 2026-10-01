# Codex integration research probes

These scripts support Brief 01A evidence collection. They are diagnostic tools,
not production integration code.

- `hook_probe.py` accepts one documented hook payload on standard input, applies
  a small fail-closed sanitizer, and appends a bounded JSON record beneath
  `/private/tmp/mochi-codex-integration-probe`. It emits no output so it cannot
  add model context or change a turn.
- `app_server_probe.py` starts the installed `codex app-server`, lists threads for
  one exact synthetic working directory, and optionally reads the newest matching
  thread. Its output contains only structure, status, counts, and hashed IDs. It
  deliberately omits prompts, responses, commands, output, paths, diffs, names,
  and account data.

The hook logger's sanitizer only establishes the controlled experiment's
privacy boundary. It is not the production sanitizer required by the product
security specification.
