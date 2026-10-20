# Test evidence v1

The four result statuses are intentionally distinct:

- `PASS`: expected observation matched.
- `FAIL`: executed and contradicted expectations.
- `BLOCKED`: execution could not safely proceed due to missing preconditions.
- `SKIP`: intentionally excluded by selection/configuration.

Consumers must not treat `BLOCKED` and `SKIP` as successful execution. Future semantic validation should require a non-empty `reason` for BLOCKED, SKIP and FAIL. `source_revision` must eventually be a verifiable commit/release identifier; synthetic examples are clearly labeled. No runner or report generator is implemented in this PR.
