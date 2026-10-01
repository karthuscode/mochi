# Brief 10 — Production normalization hardening

Authorized as part of the 2026-10-01 internal CLI MVP. Dependencies: authorized bridge 09, validated assembly 04, path/redaction 05/06. Outcome: provider JSON cannot smuggle duplicate-key ambiguity or unbounded reads into durable normalized evidence.

Current adapter maps proven hook events, preserves source correlation and safe unsupported gaps, and has independent spool/SQLite validation. Keep its provider boundary and existing support matrix. Strictly reject duplicate keys/deep/oversized input before interpreting typed fields; unknown hook names become a fixed safe metadata label. Spool reads must use no-follow regular-file opens with a byte cap, including after filename metadata races; keep scan memory bounded to 100 candidates. Never persist original malformed bytes or infer missing completion/order.

Changes: `crates/privacy` shared bounded JSON parser, `crates/capture` adapter/spool guards and synthetic failure fixtures. No new provider, assembly lifecycle, network or UI. Existing normalization/duplicate/privacy/replay tests plus duplicate nested keys, malicious unknown names, symlink and oversized file fixtures must pass. Record outcomes at the capture-foundation checkpoint and run repository gates there. This is a hardening step, not a complete Desktop capture claim.
