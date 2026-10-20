# Test evidence v1

The four result statuses are intentionally distinct:

- `PASS`: expected observation matched.
- `FAIL`: executed and contradicted expectations.
- `BLOCKED`: execution could not safely proceed due to missing preconditions.
- `SKIP`: intentionally excluded by selection/configuration.

Consumers must not treat `BLOCKED` and `SKIP` as successful execution. Future semantic validation should require a non-empty `reason` for BLOCKED, SKIP and FAIL. `source_revision` must eventually be a verifiable commit/release identifier; synthetic examples are clearly labeled. No runner or report generator is implemented in this PR.

Schema v1 now enforces a non-whitespace `reason` on FAIL, BLOCKED and SKIP. Aggregators MUST count only `PASS` as successful execution; BLOCKED/SKIP MUST be reported separately, and neither can increment an executed-pass count. A future runner must record an immutable source revision, runner version, environment, timestamps and artifact/trace references for reproducibility. No aggregation implementation is present yet.
